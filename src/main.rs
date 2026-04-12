// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! bups — print server for USB-based label printers.
//!
//! Exposes USB-connected Brother PT/QL printers over the network via:
//! - TCP port 9100 (raw print data with bidirectional status)
//! - mDNS/Bonjour advertisement for automatic discovery
//! - SNMP port 161 for status queries

mod config;
mod error;
mod health;
mod pid;
mod server;
mod status;
mod usb;

use std::sync::Arc;

use clap::Parser;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use config::{DEFAULT_MAX_RECONNECT_ATTEMPTS, DEFAULT_RECONNECT_INTERVAL, SNMP_PORT, TCP_PORT};
use usb::{Printer, list_printers};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Clone)]
#[command(
    name = "bups",
    about = "bups - the print server for USB-based label printers",
    version
)]
struct Args {
    /// TCP port for print data.
    #[arg(short, long, default_value_t = TCP_PORT)]
    port: u16,

    /// SNMP port for status queries.
    #[arg(long, default_value_t = SNMP_PORT)]
    snmp_port: u16,

    /// Enable debug logging.
    #[arg(short, long)]
    debug: bool,

    /// Bind address.
    #[arg(short, long, default_value = "[::]")]
    bind: String,

    /// Filter by model name (e.g. PT-E550W).
    #[arg(long)]
    model: Option<String>,

    /// Filter by serial number.
    #[arg(long)]
    serial: Option<String>,

    /// Hostname for mDNS advertisement.
    #[arg(long)]
    hostname: Option<String>,

    /// List connected printers and exit.
    #[arg(short, long)]
    list: bool,

    /// Reconnect check interval in seconds.
    #[arg(long, default_value_t = DEFAULT_RECONNECT_INTERVAL)]
    reconnect_interval: u64,

    /// Max reconnect attempts (0 = infinite).
    #[arg(long, default_value_t = DEFAULT_MAX_RECONNECT_ATTEMPTS)]
    max_reconnects: u32,

    /// Write PID to file (for daemon mode).
    #[arg(long)]
    pid_file: Option<String>,

    /// Log to syslog instead of stderr.
    #[arg(long)]
    syslog: bool,
}

// ---------------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------------

fn init_stderr_logging(env_filter: EnvFilter) {
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .init();
}

#[cfg(unix)]
fn init_syslog_logging(env_filter: EnvFilter) {
    use syslog_tracing::{Facility, Options, Syslog};

    let identity = c"bups";
    let options = Options::LOG_PID | Options::LOG_NDELAY;

    if let Some(syslog) = Syslog::new(identity, options, Facility::Daemon) {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(syslog)
                    .with_ansi(false)
                    .without_time(),
            )
            .init();
    } else {
        eprintln!("Failed to initialize syslog, falling back to stderr");
        init_stderr_logging(env_filter);
    }
}

fn setup_logging(args: &Args) {
    let default = if args.debug {
        "bups=debug"
    } else {
        "bups=info"
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));

    if args.syslog {
        #[cfg(unix)]
        init_syslog_logging(filter);
        #[cfg(not(unix))]
        {
            eprintln!("Syslog not supported on this platform");
            init_stderr_logging(filter);
        }
    } else {
        init_stderr_logging(filter);
    }
}

// ---------------------------------------------------------------------------
// Shutdown
// ---------------------------------------------------------------------------

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!(error = %e, "failed to listen for Ctrl+C");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let args = Args::parse();
    setup_logging(&args);

    // List mode — print and exit.
    if args.list {
        for (model, serial) in list_printers() {
            println!("{model}\t{serial}");
        }
        return;
    }

    // PID file guard (dropped on exit).
    let _pid_guard = if let Some(ref path) = args.pid_file {
        match pid::PidFileGuard::create(path) {
            Ok(guard) => Some(guard),
            Err(e) => {
                error!(error = %e, "PID file error");
                return;
            }
        }
    } else {
        None
    };

    info!("bups {} starting", env!("CARGO_PKG_VERSION"));

    // Open printer (or start without one).
    let printer: Arc<Mutex<Option<Printer>>> =
        match Printer::open(args.model.as_deref(), args.serial.as_deref()) {
            Ok(p) => {
                info!(
                    model = p.device().model_name(),
                    serial = p.serial(),
                    "printer connected"
                );
                Arc::new(Mutex::new(Some(p)))
            }
            Err(e) => {
                warn!(error = %e, "no printer, waiting for connection");
                Arc::new(Mutex::new(None))
            }
        };

    // mDNS channel — carries current (Device, serial) or None.
    let (mdns_tx, mdns_rx) = tokio::sync::watch::channel({
        let p = printer.lock().await;
        p.as_ref().map(|p| (p.device(), p.serial().to_owned()))
    });

    // Health-check / reconnect task.
    let printer_health = Arc::clone(&printer);
    let health_model = args.model.clone();
    let health_serial = args.serial.clone();
    tokio::spawn(async move {
        health::run(
            printer_health,
            health_model,
            health_serial,
            args.reconnect_interval,
            args.max_reconnects,
            mdns_tx,
        )
        .await;
    });

    // mDNS task.
    let mdns_port = args.port;
    let mdns_hostname = args.hostname.clone();
    tokio::spawn(async move {
        server::mdns::mdns_loop(mdns_rx, mdns_port, mdns_hostname).await;
    });

    // SNMP task.
    let snmp_addr = format!(
        "{}:{}",
        args.bind.trim_matches(|c| c == '[' || c == ']'),
        args.snmp_port
    );
    let printer_snmp = Arc::clone(&printer);
    tokio::spawn(async move {
        if let Err(e) = server::snmp::serve(&snmp_addr, printer_snmp).await {
            error!(error = %e, "SNMP server error");
        }
    });

    // TCP server (foreground) + shutdown signal.
    let tcp_addr = format!("{}:{}", args.bind, args.port);
    tokio::select! {
        result = server::tcp::serve(&tcp_addr, printer) => {
            if let Err(e) = result {
                error!(error = %e, "TCP server error");
            }
        }
        () = shutdown_signal() => {
            info!("shutdown signal received");
        }
    }

    info!("bups stopped");
}
