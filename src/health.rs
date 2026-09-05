// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! USB health-check / reconnect loop.
//!
//! Monitors the printer connection via USB hotplug events (with polling
//! fallback) and notifies the mDNS subsystem on connect/disconnect.

use std::sync::Arc;
use std::time::Duration;

use nusb::hotplug::HotplugEvent;
use tokio::sync::Mutex;
use tokio_stream::StreamExt;
use tracing::{debug, error, info, warn};

use crate::config::BROTHER_VENDOR_ID;
use crate::usb::{Device, Printer};

/// Run the health-check loop until the process exits.
pub async fn run(
    printer: Arc<Mutex<Option<Printer>>>,
    model_filter: Option<String>,
    serial_filter: Option<String>,
    reconnect_interval: u64,
    max_reconnects: u32,
    mdns_tx: tokio::sync::watch::Sender<Option<(Device, String)>>,
) {
    let mut hotplug = match nusb::watch_devices() {
        Ok(w) => Some(w),
        Err(e) => {
            warn!(error = %e, "USB hotplug unavailable, polling");
            None
        }
    };

    let poll = Duration::from_secs(reconnect_interval);
    let mut attempts: u32 = 0;

    loop {
        {
            let mut guard = printer.lock().await;

            // Check if current printer is still alive.
            if let Some(ref p) = *guard
                && p.read().await.is_err()
            {
                info!("printer disconnected");
                *guard = None;
                let _ = mdns_tx.send(None);
                attempts = 0;
            }

            // Try to reconnect if absent.
            if guard.is_none() {
                if max_reconnects > 0 && attempts >= max_reconnects {
                    error!(attempts, "max reconnect attempts reached");
                } else {
                    attempts += 1;
                    if let Ok(p) = Printer::open(model_filter.as_deref(), serial_filter.as_deref())
                    {
                        info!(
                            model = p.device().model_name(),
                            serial = p.serial(),
                            "printer connected"
                        );
                        let _ = mdns_tx.send(Some((p.device(), p.serial().to_owned())));
                        *guard = Some(p);
                        drop(guard);
                        attempts = 0;
                    }
                }
            }
        }

        // Wait for USB event or poll timeout.
        if let Some(ref mut watch) = hotplug {
            tokio::select! {
                event = StreamExt::next(watch) => {
                    if let Some(event) = event {
                        match event {
                            HotplugEvent::Connected(info) => {
                                if info.vendor_id() == BROTHER_VENDOR_ID {
                                    debug!(
                                        pid = info.product_id(),
                                        "Brother device connected"
                                    );
                                }
                            }
                            HotplugEvent::Disconnected(id) => {
                                debug!(?id, "USB device disconnected");
                            }
                        }
                    }
                }
                () = tokio::time::sleep(poll) => {}
            }
        } else {
            tokio::time::sleep(poll).await;
        }
    }
}
