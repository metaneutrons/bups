// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! TCP print server (port 9100).
//!
//! Accepts raw print data and forwards it to the USB printer.
//!
//! # Why the printer is held for a whole connection
//!
//! A print job is a byte stream of raster commands with length prefixes. The
//! previous version took the printer mutex per 8 KiB chunk and released it in
//! between, which left two windows open on every chunk boundary: the health
//! loop and the SNMP responder could each acquire the printer and send
//! `ESC i S`. Those three bytes then land inside a raster payload, the printer
//! keeps reading its declared length, and everything after that shifts. The
//! Brother raster protocol has no resynchronisation. CUPS polls SNMP as a
//! matter of course, so this was the normal case rather than a race.
//!
//! The printer is therefore held from the first byte of a connection until it
//! closes. That is what an idle timeout is for: without one, a peer that opens
//! a connection and sends nothing would hold the printer indefinitely.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::{debug, error, info, instrument, trace, warn};

use crate::config::{TCP_BUFFER_SIZE, TCP_IDLE_TIMEOUT_S, TCP_MAX_CONNECTIONS};
use crate::error::Result;
use crate::status::Status;
use crate::usb::Printer;

/// Start TCP server on the given address.
pub async fn serve(addr: &str, printer: Arc<Mutex<Option<Printer>>>) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!(addr = %addr, "TCP server listening");

    // Only one connection can hold the printer anyway; the cap exists so a
    // peer cannot spawn unbounded tasks, each with its own buffer.
    let live = Arc::new(AtomicUsize::new(0));

    loop {
        match listener.accept().await {
            Ok((mut stream, peer)) => {
                if live.load(Ordering::Relaxed) >= TCP_MAX_CONNECTIONS {
                    warn!(peer = %peer, "connection limit reached, refusing");
                    let _ = stream.write_all(b"ERROR: Too many connections\n").await;
                    let _ = stream.shutdown().await;
                    continue;
                }
                live.fetch_add(1, Ordering::Relaxed);
                info!(peer = %peer, "connection accepted");
                let printer = Arc::clone(&printer);
                let live = Arc::clone(&live);
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, printer).await {
                        debug!(error = %e, "connection closed");
                    }
                    live.fetch_sub(1, Ordering::Relaxed);
                });
            }
            Err(e) => error!(error = %e, "accept failed"),
        }
    }
}

/// Read the next chunk, or `None` when the client is done or has gone quiet.
async fn next_chunk(stream: &mut TcpStream, buf: &mut [u8]) -> Result<Option<usize>> {
    let idle = Duration::from_secs(TCP_IDLE_TIMEOUT_S);
    match tokio::time::timeout(idle, stream.read(buf)).await {
        Err(_) => {
            warn!(seconds = TCP_IDLE_TIMEOUT_S, "idle timeout, dropping");
            Ok(None)
        }
        Ok(Err(e)) => Err(e.into()),
        Ok(Ok(0)) => {
            debug!("client disconnected");
            Ok(None)
        }
        Ok(Ok(n)) => Ok(Some(n)),
    }
}

// clippy::significant_drop_tightening wants the printer guard released
// earlier. Holding it for the whole job is the entire point of this function,
// see the module comment.
#[allow(
    clippy::significant_drop_tightening,
    reason = "the printer lock is held for the whole job on purpose"
)]
#[instrument(skip_all, fields(peer = %stream.peer_addr().map_or_else(|_| "unknown".into(), |a| a.to_string())))]
async fn handle_connection(
    mut stream: TcpStream,
    printer: Arc<Mutex<Option<Printer>>>,
) -> Result<()> {
    let mut buf = [0u8; TCP_BUFFER_SIZE];

    // First read decides what this connection is. A text command is answered
    // without ever taking the printer, so a status query cannot block a job.
    let Some(n) = next_chunk(&mut stream, &mut buf).await? else {
        return Ok(());
    };
    if let Some(response) = handle_command(&buf[..n], &printer).await {
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.flush().await;
        return Ok(());
    }

    // Data phase. The printer is taken once and held until the client is done.
    let guard = printer.lock().await;
    let Some(ref p) = *guard else {
        let _ = stream.write_all(b"ERROR: No printer connected\n").await;
        return Ok(());
    };

    let mut written = 0usize;
    let mut chunk = Some(n);
    while let Some(n) = chunk {
        if let Err(e) = p.write(&buf[..n]).await {
            error!(error = %e, "USB write failed");
            let _ = stream.write_all(b"ERROR: USB write failed\n").await;
            return Ok(());
        }
        written += n;

        // Forward status the printer volunteered, without asking for it. One
        // short look, no retry loop: during a raster transfer there is nothing
        // to read, and waiting for it would cost seconds per chunk.
        if let Some(raw) = p.poll_status().await {
            if let Some(s) = Status::parse(raw, p.device().series()) {
                debug!(status = %s, "printer status");
                if s.has_error() {
                    warn!(errors = ?s.errors(), "printer error");
                }
            }
            let _ = stream.write_all(&raw).await;
            let _ = stream.flush().await;
        } else {
            trace!("no status pending");
        }

        chunk = next_chunk(&mut stream, &mut buf).await?;
    }

    // End of job. Now that nothing else is in flight, asking is safe.
    debug!(bytes = written, "job complete, requesting final status");
    if let Ok(raw) = p.read().await {
        let _ = stream.write_all(&raw).await;
        let _ = stream.flush().await;
    }

    Ok(())
}

async fn handle_command(data: &[u8], printer: &Arc<Mutex<Option<Printer>>>) -> Option<String> {
    // Only ever a short line. Anything larger is print data, and decoding
    // megabytes of raster as UTF-8 to compare it against two words is waste.
    if data.len() > 64 {
        return None;
    }
    let cmd = std::str::from_utf8(data).ok()?.trim().to_uppercase();

    match cmd.as_str() {
        "STATUS" => {
            let guard = printer.lock().await;
            let Some(ref p) = *guard else {
                return Some("STATUS: No printer connected\n".into());
            };
            let model = p.device().model_name();
            let series = p.device().series();
            let result = p.read().await;
            drop(guard);

            result.map_or_else(
                |_| Some("STATUS: Read failed\n".into()),
                |raw| {
                    Status::parse(raw, series).map_or_else(
                        || Some("STATUS: Unknown\n".into()),
                        |s| {
                            let label = if s.has_error() { "ERROR" } else { "READY" };
                            let errors = s.errors();
                            // errors() never comes back empty while has_error()
                            // is true: an undocumented bit is reported as its
                            // raw value rather than dropped.
                            let error_str = if errors.is_empty() {
                                "None".to_owned()
                            } else {
                                errors.join(", ")
                            };
                            Some(format!(
                                "STATUS: {label}\nModel: {model}\n\
                                 Media: {}\nError: {error_str}\n",
                                s.media_description()
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

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "panicking is how a test reports failure"
)]
mod tests {
    use super::*;

    /// No printer attached, which is all these tests need: the command path
    /// must answer without ever touching hardware.
    fn no_printer() -> Arc<Mutex<Option<Printer>>> {
        Arc::new(Mutex::new(None))
    }

    #[tokio::test]
    async fn status_is_recognised_and_answered_without_a_printer() {
        let out = handle_command(b"STATUS\n", &no_printer()).await;
        assert_eq!(out.as_deref(), Some("STATUS: No printer connected\n"));
    }

    #[tokio::test]
    async fn commands_are_case_insensitive_and_trimmed() {
        for raw in ["status", "  Status \r\n", "STATUS"] {
            let out = handle_command(raw.as_bytes(), &no_printer()).await;
            assert!(out.is_some(), "{raw:?} was not recognised");
        }
    }

    #[tokio::test]
    async fn help_lists_the_commands() {
        let out = handle_command(b"HELP\n", &no_printer())
            .await
            .expect("HELP is a command");
        assert!(out.contains("STATUS"));
        assert!(out.contains("HELP"));
    }

    /// Anything that is not a command must fall through to the data path,
    /// otherwise print data would be swallowed.
    #[tokio::test]
    async fn print_data_is_not_mistaken_for_a_command() {
        // ESC @ followed by raster bytes.
        assert!(
            handle_command(b"\x1b@\x00\x01\x02", &no_printer())
                .await
                .is_none()
        );
        assert!(handle_command(b"PRINT", &no_printer()).await.is_none());
        assert!(handle_command(b"", &no_printer()).await.is_none());
        // Invalid UTF-8 must not panic.
        assert!(
            handle_command(&[0xff, 0xfe, 0xfd], &no_printer())
                .await
                .is_none()
        );
    }

    /// A command is a short line. Decoding megabytes of raster as UTF-8 to
    /// compare it against two words would be waste, and a large chunk that
    /// happened to decode could otherwise swallow a job.
    #[tokio::test]
    async fn a_long_chunk_is_never_a_command() {
        let mut long = b"STATUS".to_vec();
        long.resize(65, b' ');
        assert_eq!(long.len(), 65);
        assert!(handle_command(&long, &no_printer()).await.is_none());

        let mut short = b"STATUS".to_vec();
        short.resize(64, b' ');
        assert!(
            handle_command(&short, &no_printer()).await.is_some(),
            "64 bytes is still within the command window"
        );
    }
}
