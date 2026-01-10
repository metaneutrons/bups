// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! TCP print server (port 9100).
//!
//! Accepts raw print data and forwards to USB printer.
//! Returns 32-byte status responses after each write (Brother protocol).

use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::{debug, error, info, trace, warn};

use crate::config::{POST_WRITE_DELAY_MS, TCP_BUFFER_SIZE};
use crate::error::Result;
use crate::status::Status;
use crate::usb::Printer;

/// Start TCP server on given address.
pub async fn serve(addr: &str, printer: Arc<Mutex<Option<Printer>>>) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!(addr = %addr, "TCP server listening");

    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                info!(peer = %peer, "connection accepted");
                let printer = Arc::clone(&printer);
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, printer).await {
                        // TODO: investigate if connection reset is normal P-Touch behavior
                        debug!(error = %e, "connection closed");
                    }
                });
            }
            Err(e) => error!(error = %e, "accept failed"),
        }
    }
}

async fn handle_connection(mut stream: TcpStream, printer: Arc<Mutex<Option<Printer>>>) -> Result<()> {
    let mut buf = [0u8; TCP_BUFFER_SIZE];
    let mut last_status = [0u8; 32];

    loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            debug!("connection closed");
            break;
        }

        debug!(bytes = n, "received data");

        // Check for text commands
        if let Some(response) = handle_command(&buf[..n], &printer).await {
            let _ = stream.write_all(response.as_bytes()).await;
            continue;
        }

        let guard = printer.lock().await;
        let Some(ref p) = *guard else {
            let _ = stream.write_all(b"ERROR: No printer connected\n").await;
            continue;
        };

        // Write to printer
        if let Err(e) = p.write(&buf[..n]).await {
            error!(error = %e, "USB write failed");
            drop(guard);
            let _ = stream.write_all(b"ERROR: USB write failed\n").await;
            continue;
        }
        
        // Try to read status, cache last good one
        tokio::time::sleep(Duration::from_millis(POST_WRITE_DELAY_MS)).await;
        let read_ok = if let Ok(raw) = p.read_raw().await {
            debug!(raw = ?&raw[..8], "raw status bytes");
            if let Some(s) = Status::parse(raw) {
                debug!(status_type = raw[18], phase = raw[20], "printer status");
                if s.error_message().is_some() {
                    warn!(error = ?s.error_message(), "printer error");
                }
            }
            last_status = raw;
            true
        } else {
            // FIXME: nusb can't read status after print command - workaround by faking ready status
            if n <= 4 && last_status[18] == 6 {
                last_status[18] = 0; // Set status_type to ready
                last_status[20] = 0; // Set phase to 0
            }
            false
        };
        drop(guard);
        
        // Always send status (last good or cached)
        trace!(status = ?&last_status[..], fresh = read_ok, "sending status to client");
        let _ = stream.write_all(&last_status).await;
        let _ = stream.flush().await;
    }

    Ok(())
}

async fn handle_command(data: &[u8], printer: &Arc<Mutex<Option<Printer>>>) -> Option<String> {
    let cmd = std::str::from_utf8(data).ok()?.trim().to_uppercase();
    
    match cmd.as_str() {
        "STATUS" => {
            let guard = printer.lock().await;
            let Some(ref p) = *guard else {
                return Some("STATUS: No printer connected\n".into());
            };
            match p.read().await {
                Ok(raw) => {
                    if let Some(s) = Status::parse(raw) {
                        let status = if s.error_message().is_some() { "ERROR" } else { "READY" };
                        Some(format!(
                            "STATUS: {}\nModel: {}\nMedia: {}mm\nError: {}\n",
                            status,
                            p.device().model_name(),
                            s.media_width,
                            s.error_message().unwrap_or("None")
                        ))
                    } else {
                        Some("STATUS: Unknown\n".into())
                    }
                }
                Err(_) => Some("STATUS: Read failed\n".into()),
            }
        }
        "HELP" => Some(
            "Commands:\n  STATUS - Get printer status\n  HELP   - Show this help\n  <data> - Send raw print data\n".into()
        ),
        _ => None,
    }
}
