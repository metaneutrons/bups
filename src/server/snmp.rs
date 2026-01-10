// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! SNMP responder for printer status queries.
//!
//! Responds to Brother-specific OID (1.3.6.1.4.1.2435.3.3.9.1.6.1.0)
//! with raw 32-byte printer status.

use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use rasn::types::{Integer, ObjectIdentifier, OctetString};
use rasn_snmp::v1::{GetRequest, GetResponse, Message, Pdu, Pdus, VarBind};
use rasn_smi::v1::{ObjectSyntax, SimpleSyntax};

use crate::config::BROTHER_STATUS_OID;
use crate::error::Result;
use crate::usb::Printer;

const SNMP_BUFFER_SIZE: usize = 1024;

/// Start SNMP responder on given address.
pub async fn serve(addr: &str, printer: Arc<Mutex<Option<Printer>>>) -> Result<()> {
    let socket = UdpSocket::bind(addr).await?;
    info!(addr = %addr, "SNMP responder listening");

    let brother_oid = ObjectIdentifier::new_unchecked(BROTHER_STATUS_OID.to_vec().into());
    let mut buf = [0u8; SNMP_BUFFER_SIZE];

    loop {
        let (len, src) = match socket.recv_from(&mut buf).await {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "SNMP recv error");
                continue;
            }
        };

        debug!(src = %src, len, "SNMP request");

        // Decode request
        let Ok(msg) = rasn::ber::decode::<Message<Pdus>>(&buf[..len]) else {
            continue;
        };

        let Pdus::GetRequest(GetRequest(pdu)) = msg.data else {
            continue;
        };

        // Log requested OID
        if let Some(vb) = pdu.variable_bindings.first() {
            debug!(oid = ?vb.name, "SNMP requested OID");
        }

        // Get printer status - use try_lock to avoid blocking during print jobs
        let status_bytes = {
            match printer.try_lock() {
                Ok(guard) => {
                    if let Some(ref p) = *guard {
                        p.read().await.ok().map(|s| s.to_vec())
                    } else {
                        None
                    }
                }
                Err(_) => None, // Printer busy
            }
        };
        
        // Skip response if no status available (printer busy)
        let Some(status_bytes) = status_bytes else {
            continue;
        };

        // Build response
        let response = Message {
            version: Integer::from(0),
            community: msg.community,
            data: GetResponse(Pdu {
                request_id: pdu.request_id,
                error_status: Integer::from(0),
                error_index: Integer::from(0),
                variable_bindings: vec![VarBind {
                    name: brother_oid.clone(),
                    value: ObjectSyntax::Simple(SimpleSyntax::String(
                        OctetString::from(status_bytes),
                    )),
                }],
            }),
        };

        if let Ok(encoded) = rasn::ber::encode(&response) {
            let _ = socket.send_to(&encoded, src).await;
            debug!(src = %src, len = encoded.len(), "SNMP response sent");
        }
    }
}
