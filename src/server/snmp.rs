// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! SNMP responder for printer status queries.
//!
//! Responds to the Brother-specific OID (`1.3.6.1.4.1.2435.3.3.9.1.6.1.0`)
//! with the raw 32-byte printer status.

use std::sync::Arc;

use rasn::types::{Integer, ObjectIdentifier, OctetString};
use rasn_smi::v1::{ObjectSyntax, SimpleSyntax};
use rasn_snmp::v1::{GetRequest, GetResponse, Message, Pdu, Pdus, VarBind};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::config::{BROTHER_STATUS_OID, SNMP_BUFFER_SIZE};
use crate::error::Result;
use crate::usb::Printer;

/// Start SNMP responder on the given address.
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

        let msg = match rasn::ber::decode::<Message<Pdus>>(&buf[..len]) {
            Ok(m) => m,
            Err(e) => {
                warn!(src = %src, error = %e, "malformed SNMP request");
                continue;
            }
        };

        let Pdus::GetRequest(GetRequest(pdu)) = msg.data else {
            debug!(src = %src, "ignoring non-GET SNMP request");
            continue;
        };

        if let Some(vb) = pdu.variable_bindings.first() {
            debug!(oid = ?vb.name, "requested OID");
        }

        // Use try_lock to avoid blocking during print jobs.
        let status_bytes = match printer.try_lock() {
            Ok(guard) => match *guard {
                Some(ref p) => p.read().await.ok().map(|s| s.to_vec()),
                None => None,
            },
            Err(_) => None,
        };

        let Some(status_bytes) = status_bytes else {
            debug!(src = %src, "printer busy or absent, skipping response");
            continue;
        };

        let response = Message {
            version: Integer::from(0),
            community: msg.community,
            data: GetResponse(Pdu {
                request_id: pdu.request_id,
                error_status: Integer::from(0),
                error_index: Integer::from(0),
                variable_bindings: vec![VarBind {
                    name: brother_oid.clone(),
                    value: ObjectSyntax::Simple(SimpleSyntax::String(OctetString::from(
                        status_bytes,
                    ))),
                }],
            }),
        };

        if let Ok(encoded) = rasn::ber::encode(&response) {
            let _ = socket.send_to(&encoded, src).await;
            debug!(src = %src, len = encoded.len(), "SNMP response sent");
        }
    }
}
