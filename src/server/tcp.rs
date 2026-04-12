// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! TCP print server (port 9100).
//!
//! Accepts raw print data and forwards it to the USB printer.
//! Returns 32-byte status responses after each write (Brother protocol).

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::{debug, error, info, instrument, trace, warn};

use crate::config::{POST_WRITE_DELAY_MS, TCP_BUFFER_SIZE, USB_READ_SIZE};
use crate::error::Result;
use crate::status::Status;
use crate::usb::Printer;

/// Start TCP server on the given address.
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
                        debug!(error = %e, "connection closed");
                    }
                });
            }
            Err(e) => error!(error = %e, "accept failed"),
        }
    }
}

#[instrument(skip_all, fields(peer = %stream.peer_addr().map_or_else(|_| "unknown".into(), |a| a.to_string())))]
async fn handle_connection(
    mut stream: TcpStream,
    printer: Arc<Mutex<Option<Printer>>>,
) -> Result<()> {
    let mut buf = [0u8; TCP_BUFFER_SIZE];
    let mut last_status = [0u8; USB_READ_SIZE];

    loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            debug!("client disconnected");
            break;
        }

        debug!(bytes = n, "received data");

        // Check for text commands.
        if let Some(response) = handle_command(&buf[..n], &printer).await {
            let _ = stream.write_all(response.as_bytes()).await;
            continue;
        }

        let guard = printer.lock().await;
        let Some(ref p) = *guard else {
            let _ = stream.write_all(b"ERROR: No printer connected\n").await;
            continue;
        };

        // Write to printer.
        if let Err(e) = p.write(&buf[..n]).await {
            error!(error = %e, "USB write failed");
            drop(guard);
            let _ = stream.write_all(b"ERROR: USB write failed\n").await;
            continue;
        }

        // Read status after write; cache last good response.
        tokio::time::sleep(Duration::from_millis(POST_WRITE_DELAY_MS)).await;
        if let Ok(raw) = p.read_raw().await {
            if let Some(s) = Status::parse(raw) {
                debug!(status = %s, "printer status");
                if s.has_error() {
                    warn!(errors = ?s.errors(), "printer error");
                }
            }
            last_status = raw;
        } else {
            trace!("no fresh status, sending cached");
        }
        drop(guard);

        // Always send status (fresh or cached) to client.
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
            let model = p.device().model_name();
            let result = p.read().await;
            drop(guard);

            result.map_or_else(
                |_| Some("STATUS: Read failed\n".into()),
                |raw| {
                    Status::parse(raw).map_or_else(
                        || Some("STATUS: Unknown\n".into()),
                        |s| {
                            let label = if s.has_error() { "ERROR" } else { "READY" };
                            let errors = s.errors();
                            let error_str = if errors.is_empty() {
                                "None".to_owned()
                            } else {
                                errors.join(", ")
                            };
                            Some(format!(
                                "STATUS: {label}\nModel: {model}\n\
                                 Media: {}mm\nError: {error_str}\n",
                                s.media_width
                            ))
                        },
                    )
                },
            )
        }
        "HELP" => Some(
            "Commands:\n  STATUS - Get printer status\n  \
             HELP   - Show this help\n  <data> - Send raw print data\n"
                .into(),
        ),
        _ => None,
    }
}
