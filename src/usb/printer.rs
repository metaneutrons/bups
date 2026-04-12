// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! USB printer communication via nusb.
//!
//! Handles device detection, connection, and bidirectional data transfer.
//! All blocking USB I/O is dispatched via [`tokio::task::spawn_blocking`]
//! to avoid stalling the async runtime.

use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use nusb::transfer::{Bulk, In, Out};
use nusb::{Endpoint, MaybeFuture};
use tracing::{debug, trace, warn};

use crate::config::{
    BROTHER_VENDOR_ID, INIT_CMD, INVALIDATE_CMD, STATUS_MAGIC, STATUS_REQUEST, USB_EP_IN,
    USB_EP_OUT, USB_READ_BUFFER, USB_READ_RETRIES, USB_READ_SIZE, USB_READ_TIMEOUT_MS,
    USB_STATUS_DELAY_MS, USB_TIMEOUT_MS, USB_WRITE_CHUNK,
};
use crate::error::{Error, Result};
use crate::usb::Device;

/// List connected supported printers (blocking, call from sync context).
pub fn list_printers() -> Vec<(&'static str, String)> {
    let Ok(devices) = nusb::list_devices().wait() else {
        return Vec::new();
    };
    devices
        .filter(|d| d.vendor_id() == BROTHER_VENDOR_ID)
        .filter_map(|d| {
            let device = Device::from_pid(d.product_id())?;
            let serial = d.serial_number().unwrap_or("Unknown").to_string();
            Some((device.model_name(), serial))
        })
        .collect()
}

/// USB printer handle.
///
/// Endpoints are wrapped in `Arc<StdMutex<..>>` so they can be moved
/// into `spawn_blocking` closures.
pub struct Printer {
    device: Device,
    ep_out: Arc<StdMutex<Endpoint<Bulk, Out>>>,
    ep_in: Arc<StdMutex<Endpoint<Bulk, In>>>,
    serial: String,
}

impl Printer {
    /// Detect and open a printer, optionally filtering by model or serial.
    pub fn open(model_filter: Option<&str>, serial_filter: Option<&str>) -> Result<Self> {
        let mut supported: Vec<_> = nusb::list_devices()
            .wait()
            .map_err(Error::Usb)?
            .filter(|d| d.vendor_id() == BROTHER_VENDOR_ID)
            .filter_map(|info| {
                let device = Device::from_pid(info.product_id())?;
                let serial = info.serial_number().unwrap_or("Unknown").to_string();
                Some((info, device, serial))
            })
            .collect();

        if supported.is_empty() {
            return Err(Error::NoPrinter);
        }

        if supported.len() > 1 {
            warn!(
                count = supported.len(),
                "multiple printers found, use --model or --serial to select"
            );
            for (_, dev, ser) in &supported {
                warn!(model = dev.model_name(), serial = %ser, "available");
            }
        }

        if let Some(model) = model_filter {
            supported.retain(|(_, d, _)| d.model_name().eq_ignore_ascii_case(model));
        }
        if let Some(serial) = serial_filter {
            supported.retain(|(_, _, s)| s == serial);
        }
        if supported.is_empty() {
            return Err(Error::NoPrinter);
        }

        let (info, device, serial) = supported.remove(0);
        debug!(model = device.model_name(), serial = %serial, "opening printer");

        let usb = info.open().wait().map_err(Error::Usb)?;
        let iface = usb.claim_interface(0).wait().map_err(Error::Usb)?;

        let ep_out = iface
            .endpoint::<Bulk, Out>(USB_EP_OUT)
            .map_err(|e| Error::Transfer(e.to_string()))?;
        let ep_in = iface
            .endpoint::<Bulk, In>(USB_EP_IN)
            .map_err(|e| Error::Transfer(e.to_string()))?;

        let printer = Self {
            device,
            ep_out: Arc::new(StdMutex::new(ep_out)),
            ep_in: Arc::new(StdMutex::new(ep_in)),
            serial,
        };

        // Reset and initialise device.
        printer.write_sync(&INVALIDATE_CMD)?;
        printer.write_sync(&INIT_CMD)?;

        Ok(printer)
    }

    // -- Sync helpers (used during init only) --

    fn write_sync(&self, data: &[u8]) -> Result<()> {
        let mut ep = self.ep_out.lock().map_err(|_| Error::MutexPoisoned)?;
        ep.submit(data.to_vec().into());
        match ep.wait_next_complete(Duration::from_millis(USB_TIMEOUT_MS)) {
            Some(c) if c.status.is_err() => Err(Error::Transfer(format!("{:?}", c.status))),
            None => Err(Error::Transfer("USB write timeout".into())),
            _ => Ok(()),
        }
    }

    // -- Public accessors --

    #[must_use]
    pub const fn device(&self) -> Device {
        self.device
    }

    #[must_use]
    pub fn serial(&self) -> &str {
        &self.serial
    }

    // -- Async wrappers (dispatch blocking USB I/O off the runtime) --

    /// Write data to printer (chunked for QL compatibility).
    pub async fn write(&self, data: &[u8]) -> Result<()> {
        let ep = Arc::clone(&self.ep_out);
        let data = data.to_vec();
        tokio::task::spawn_blocking(move || {
            let mut ep = ep.lock().map_err(|_| Error::MutexPoisoned)?;
            for chunk in data.chunks(USB_WRITE_CHUNK) {
                trace!(len = chunk.len(), "USB write");
                ep.submit(chunk.to_vec().into());
                match ep.wait_next_complete(Duration::from_millis(USB_TIMEOUT_MS)) {
                    Some(c) if c.status.is_err() => {
                        return Err(Error::Transfer(format!("{:?}", c.status)));
                    }
                    None => {
                        return Err(Error::Transfer("USB write timeout".into()));
                    }
                    _ => {}
                }
            }
            Ok(())
        })
        .await
        .map_err(|e| Error::Transfer(e.to_string()))?
    }

    /// Read raw status bytes without sending a status request first.
    pub async fn read_raw(&self) -> Result<[u8; USB_READ_SIZE]> {
        let ep = Arc::clone(&self.ep_in);
        tokio::task::spawn_blocking(move || {
            let mut ep = ep.lock().map_err(|_| Error::MutexPoisoned)?;
            let _ = ep.clear_halt();

            for attempt in 0..USB_READ_RETRIES {
                ep.submit(vec![0u8; USB_READ_BUFFER].into());
                match ep.wait_next_complete(Duration::from_millis(USB_READ_TIMEOUT_MS)) {
                    Some(c) if c.status.is_ok() && c.actual_len > 0 => {
                        let mut buf = [0u8; USB_READ_SIZE];
                        let len = c.actual_len.min(USB_READ_SIZE);
                        buf[..len].copy_from_slice(&c.buffer[..len]);
                        if buf[0..2] == STATUS_MAGIC {
                            trace!(?buf, attempt, "valid status");
                            return Ok(buf);
                        }
                        trace!(data = ?&buf[..8], attempt, "non-status data, retrying");
                    }
                    Some(c) if c.status.is_err() => {
                        return Err(Error::Transfer(format!("{:?}", c.status)));
                    }
                    _ => trace!(attempt, "USB read empty/timeout"),
                }
            }
            Err(Error::Transfer("USB read timeout after retries".into()))
        })
        .await
        .map_err(|e| Error::Transfer(e.to_string()))?
    }

    /// Send a status request and read the response.
    pub async fn read(&self) -> Result<[u8; USB_READ_SIZE]> {
        self.write(&STATUS_REQUEST).await?;
        tokio::time::sleep(Duration::from_millis(USB_STATUS_DELAY_MS)).await;
        self.read_raw().await
    }
}
