// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! USB printer communication via nusb.
//!
//! Handles device detection, connection, and bidirectional data transfer.
//! Uses chunked writes (4KB) for compatibility with QL printers' smaller USB buffers.

use std::sync::Mutex as StdMutex;
use std::time::Duration;
use nusb::Endpoint;
use nusb::transfer::{Bulk, In, Out};
use nusb::MaybeFuture;
use tracing::{debug, trace, warn};

use crate::config::{BROTHER_VENDOR_ID, INIT_CMD, INVALIDATE_CMD, USB_READ_SIZE, USB_TIMEOUT_MS};
use crate::error::{Error, Result};
use crate::usb::Device;

/// Status request command (ESC i S).
const STATUS_REQUEST: [u8; 3] = [0x1b, 0x69, 0x53];

/// List connected supported printers.
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
/// Note: Uses std::sync::Mutex because nusb operations are blocking anyway.
/// USB operations are wrapped in spawn_blocking when called from async context.
pub struct Printer {
    device: Device,
    ep_out: StdMutex<Endpoint<Bulk, Out>>,
    ep_in: StdMutex<Endpoint<Bulk, In>>,
    serial: String,
}

impl Printer {
    /// Detect and open a printer, optionally filtering by model or serial.
    pub fn open(model_filter: Option<&str>, serial_filter: Option<&str>) -> Result<Self> {
        let devices: Vec<_> = nusb::list_devices()
            .wait()
            .map_err(Error::Usb)?
            .filter(|d| d.vendor_id() == BROTHER_VENDOR_ID)
            .collect();

        let mut supported: Vec<_> = devices
            .into_iter()
            .filter_map(|dev_info| {
                let pid = dev_info.product_id();
                let device = Device::from_pid(pid)?;
                let serial = dev_info.serial_number().unwrap_or("Unknown").to_string();
                Some((dev_info, device, serial))
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
                warn!(model = dev.model_name(), serial = %ser, "available printer");
            }
        }

        if let Some(model) = model_filter {
            supported.retain(|(_, dev, _)| dev.model_name().eq_ignore_ascii_case(model));
            if supported.is_empty() {
                return Err(Error::NoPrinter);
            }
        }

        if let Some(serial) = serial_filter {
            supported.retain(|(_, _, ser)| ser == serial);
            if supported.is_empty() {
                return Err(Error::NoPrinter);
            }
        }

        let (dev_info, device, serial) = supported.remove(0);
        debug!(model = device.model_name(), serial = %serial, "opening printer");

        let usb_device = dev_info.open().wait().map_err(Error::Usb)?;
        let interface = usb_device.claim_interface(0).wait().map_err(Error::Usb)?;

        let ep_out = interface.endpoint::<Bulk, Out>(0x02)
            .map_err(|e| Error::Transfer(e.to_string()))?;
        let ep_in = interface.endpoint::<Bulk, In>(0x81)
            .map_err(|e| Error::Transfer(e.to_string()))?;

        let printer = Self {
            device,
            ep_out: StdMutex::new(ep_out),
            ep_in: StdMutex::new(ep_in),
            serial,
        };

        // Reset and initialize device
        printer.write_sync(&INVALIDATE_CMD)?;
        printer.write_sync(&INIT_CMD)?;

        Ok(printer)
    }

    fn write_sync(&self, data: &[u8]) -> Result<()> {
        let mut ep = self.ep_out.lock().unwrap();
        ep.submit(data.to_vec().into());
        match ep.wait_next_complete(Duration::from_millis(USB_TIMEOUT_MS)) {
            Some(c) if c.status.is_err() => Err(Error::Transfer(format!("{:?}", c.status))),
            None => Err(Error::Transfer("USB write timeout".into())),
            _ => Ok(()),
        }
    }

    pub fn device(&self) -> Device {
        self.device
    }

    pub fn serial(&self) -> &str {
        &self.serial
    }

    pub async fn write(&self, data: &[u8]) -> Result<()> {
        const CHUNK_SIZE: usize = 4096; // QL printers have smaller USB buffers
        let mut ep = self.ep_out.lock().unwrap();
        
        for chunk in data.chunks(CHUNK_SIZE) {
            trace!(len = chunk.len(), "USB write");
            ep.submit(chunk.to_vec().into());
            match ep.wait_next_complete(Duration::from_millis(USB_TIMEOUT_MS)) {
                Some(c) if c.status.is_err() => return Err(Error::Transfer(format!("{:?}", c.status))),
                None => return Err(Error::Transfer("USB write timeout".into())),
                _ => {}
            }
        }
        Ok(())
    }

    /// Read raw data from printer without sending status request.
    pub async fn read_raw(&self) -> Result<[u8; USB_READ_SIZE]> {
        let mut ep_in = self.ep_in.lock().unwrap();
        
        // Clear any halt condition
        let _ = ep_in.clear_halt();
        
        for attempt in 0..10 {
            ep_in.submit(vec![0u8; 64].into());
            match ep_in.wait_next_complete(Duration::from_millis(500)) {
                Some(c) if c.status.is_ok() && c.actual_len > 0 => {
                    let mut buf = [0u8; USB_READ_SIZE];
                    let len = c.actual_len.min(USB_READ_SIZE);
                    buf[..len].copy_from_slice(&c.buffer[..len]);
                    // Check for valid status header (0x80, 0x20)
                    if buf[0] == 0x80 && buf[1] == 0x20 {
                        trace!(data = ?buf, attempt, "USB read valid status");
                        return Ok(buf);
                    }
                    trace!(data = ?&buf[..8], attempt, "USB read non-status data, retrying");
                }
                Some(c) if c.status.is_err() => {
                    return Err(Error::Transfer(format!("{:?}", c.status)));
                }
                _ => trace!(attempt, "USB read empty/timeout"),
            }
        }
        Err(Error::Transfer("USB read timeout after retries".into()))
    }

    /// Read status from printer (sends status request first).
    pub async fn read(&self) -> Result<[u8; USB_READ_SIZE]> {
        self.write(&STATUS_REQUEST).await?;
        tokio::time::sleep(Duration::from_millis(50)).await;
        self.read_raw().await
    }
}
