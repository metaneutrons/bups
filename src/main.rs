// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! bups - print server for USB-based label printers.
//!
//! Exposes USB-connected Brother PT/QL printers over the network via:
//! - TCP port 9100 (raw print data with bidirectional status)
//! - mDNS/Bonjour advertisement for automatic discovery
//! - SNMP port 161 for status queries

mod config;
mod error;
mod server;
mod status;
mod usb;

use std::sync::Arc;
use std::time::Duration;
use clap::Parser;
use tokio::sync::Mutex;
use tokio::signal;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use config::{DEFAULT_MAX_RECONNECT_ATTEMPTS, DEFAULT_RECONNECT_INTERVAL, TCP_PORT, SNMP_PORT};
use usb::{Printer, list_printers};

fn init_stderr_logging(env_filter: EnvFilter) {
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .init();
}

#[cfg(unix)]
fn init_syslog_logging(env_filter: EnvFilter) {
    use syslog_tracing::{Syslog, Options, Facility};

    let identity = c"bups";
    let options = Options::LOG_PID | Options::LOG_NDELAY;

    match Syslog::new(identity, options, Facility::Daemon) {
        Some(syslog) => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_writer(syslog)
                        .with_ansi(false)
                        .without_time()
                )
                .init();
        }
        None => {
            eprintln!("Failed to initialize syslog (already initialized?), falling back to stderr");
            init_stderr_logging(env_filter);
        }
    }
}

struct PidFileGuard(String);

impl PidFileGuard {
    fn create(path: &str) -> std::result::Result<Self, String> {
        // Check if PID file exists and process is running
        if let Ok(contents) = std::fs::read_to_string(path) {
            if let Ok(pid) = contents.trim().parse::<i32>() {
                // Check if process is still running (kill -0)
                #[cfg(unix)]
                {
                    // SAFETY: kill with signal 0 just checks if process exists
                    let exists = unsafe { libc::kill(pid, 0) == 0 };
                    if exists {
                        return Err(format!("another instance already running (PID {pid})"));
                    }
                }
            }
            // Stale PID file, remove it
            let _ = std::fs::remove_file(path);
        }

        // Write new PID file
        std::fs::write(path, std::process::id().to_string())
            .map_err(|e| format!("failed to write PID file: {e}"))?;

        Ok(Self(path.to_string()))
    }
}

impl Drop for PidFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[derive(Parser, Clone)]
#[command(name = "bups", about = "bups - the print server for USB-based label printers")]
struct Args {
    /// TCP port for print data
    #[arg(short, long, default_value_t = TCP_PORT)]
    port: u16,

    /// SNMP port for status queries
    #[arg(long, default_value_t = SNMP_PORT)]
    snmp_port: u16,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Bind address
    #[arg(short, long, default_value = "[::]")]
    bind: String,

    /// Filter by model name (e.g. PT-E550W)
    #[arg(long)]
    model: Option<String>,

    /// Filter by serial number
    #[arg(long)]
    serial: Option<String>,

    /// Hostname for mDNS advertisement
    #[arg(long)]
    hostname: Option<String>,

    /// List connected printers and exit
    #[arg(short, long)]
    list: bool,

    /// Reconnect check interval in seconds
    #[arg(long, default_value_t = DEFAULT_RECONNECT_INTERVAL)]
    reconnect_interval: u64,

    /// Max reconnect attempts (0 = infinite)
    #[arg(long, default_value_t = DEFAULT_MAX_RECONNECT_ATTEMPTS)]
    max_reconnects: u32,

    /// Write PID to file
    #[arg(long)]
    pid_file: Option<String>,

    /// Log to syslog instead of stderr
    #[arg(long)]
    syslog: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Initialize logging
    let default_filter = if args.debug { "bups=debug" } else { "bups=info" };
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_filter));

    if args.syslog {
        #[cfg(unix)]
        init_syslog_logging(env_filter);
        #[cfg(not(unix))]
        {
            eprintln!("Syslog not supported on this platform");
            init_stderr_logging(env_filter);
        }
    } else {
        init_stderr_logging(env_filter);
    }

    if args.list {
        for (model, serial) in list_printers() {
            println!("{}\t{}", model, serial);
        }
        return;
    }

    // Write PID file
    let _pid_guard = if let Some(ref path) = args.pid_file {
        match PidFileGuard::create(path) {
            Ok(guard) => Some(guard),
            Err(e) => {
                error!(error = %e, "PID file error");
                return;
            }
        }
    } else {
        None
    };

    info!("bups starting");

    let printer: Arc<Mutex<Option<Printer>>> = match Printer::open(args.model.as_deref(), args.serial.as_deref()) {
        Ok(p) => {
            info!(
                model = p.device().model_name(),
                serial = p.serial(),
                "printer connected"
            );
            Arc::new(Mutex::new(Some(p)))
        }
        Err(e) => {
            warn!(error = %e, "no printer found, waiting for connection");
            Arc::new(Mutex::new(None))
        }
    };

    // Channel for mDNS re-advertisement
    let (mdns_tx, mdns_rx) = tokio::sync::watch::channel::<Option<(usb::Device, String)>>({
        let p = printer.lock().await;
        p.as_ref().map(|p| (p.device(), p.serial().to_string()))
    });

    let printer_health = Arc::clone(&printer);
    let args_health = args.clone();
    tokio::spawn(async move {
        health_check_loop(printer_health, args_health, mdns_tx).await;
    });

    // mDNS task - re-advertises when printer changes
    let mdns_args = args.clone();
    tokio::spawn(async move {
        mdns_loop(mdns_rx, mdns_args.port, mdns_args.hostname).await;
    });

    let snmp_addr = format!("{}:{}", args.bind.trim_matches(|c| c == '[' || c == ']'), args.snmp_port);
    let printer_snmp = Arc::clone(&printer);
    tokio::spawn(async move {
        if let Err(e) = server::snmp::serve(&snmp_addr, printer_snmp).await {
            error!(error = %e, "SNMP server error");
        }
    });

    let tcp_addr = format!("{}:{}", args.bind, args.port);
    
    tokio::select! {
        result = server::tcp::serve(&tcp_addr, printer) => {
            if let Err(e) = result {
                error!(error = %e, "TCP server error");
            }
        }
        _ = shutdown_signal() => {
            info!("shutdown signal received");
        }
    }
    
    info!("bups stopped");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}

async fn health_check_loop(
    printer: Arc<Mutex<Option<Printer>>>,
    args: Args,
    mdns_tx: tokio::sync::watch::Sender<Option<(usb::Device, String)>>,
) {
    use nusb::hotplug::HotplugEvent;
    use tokio_stream::StreamExt;

    // Try to set up USB hotplug watching
    let mut hotplug = match nusb::watch_devices() {
        Ok(w) => Some(w),
        Err(e) => {
            warn!(error = %e, "USB hotplug not available, falling back to polling");
            None
        }
    };

    let poll_interval = Duration::from_secs(args.reconnect_interval);
    let mut attempts: u32 = 0;

    loop {
        // Check/reconnect logic
        {
            let mut guard = printer.lock().await;

            // Check if current printer is still connected
            if let Some(ref p) = *guard {
                if p.read().await.is_err() {
                    info!("printer disconnected");
                    *guard = None;
                    let _ = mdns_tx.send(None);
                    attempts = 0;
                }
            }

            // Try to connect if no printer
            if guard.is_none() {
                if args.max_reconnects > 0 && attempts >= args.max_reconnects {
                    error!(attempts, "max reconnect attempts reached, giving up");
                } else {
                    attempts += 1;
                    if let Ok(p) = Printer::open(args.model.as_deref(), args.serial.as_deref()) {
                        info!(
                            model = p.device().model_name(),
                            serial = p.serial(),
                            "printer connected"
                        );
                        let _ = mdns_tx.send(Some((p.device(), p.serial().to_string())));
                        *guard = Some(p);
                        attempts = 0;
                    }
                }
            }
        }

        // Wait for USB event or timeout
        if let Some(ref mut watch) = hotplug {
            tokio::select! {
                event = StreamExt::next(watch) => {
                    if let Some(event) = event {
                        match event {
                            HotplugEvent::Connected(info) => {
                                if info.vendor_id() == crate::config::BROTHER_VENDOR_ID {
                                    debug!(pid = info.product_id(), "Brother device connected");
                                }
                            }
                            HotplugEvent::Disconnected(id) => {
                                debug!(?id, "USB device disconnected");
                            }
                        }
                    }
                }
                _ = tokio::time::sleep(poll_interval) => {}
            }
        } else {
            tokio::time::sleep(poll_interval).await;
        }
    }
}

async fn mdns_loop(
    mut rx: tokio::sync::watch::Receiver<Option<(usb::Device, String)>>,
    port: u16,
    hostname: Option<String>,
) {
    use mdns_sd::ServiceDaemon;
    
    // Keep ServiceDaemon alive - dropping it unregisters the service
    let mut _mdns: Option<ServiceDaemon> = None;
    let mut current: Option<(usb::Device, String)> = None;
    
    // Handle initial value
    {
        let value = rx.borrow_and_update().clone();
        if let Some((device, ref serial)) = value {
            _mdns = server::mdns::advertise(device, serial, port, hostname.as_deref());
            current = value;
        }
    }
    
    // Handle changes
    while rx.changed().await.is_ok() {
        let value = rx.borrow().clone();
        
        // Skip if unchanged
        if value == current {
            continue;
        }
        current = value.clone();
        
        // Drop old advertisement
        _mdns = None;
        
        // Create new advertisement if printer connected
        if let Some((device, ref serial)) = value {
            _mdns = server::mdns::advertise(device, serial, port, hostname.as_deref());
        }
    }
}
