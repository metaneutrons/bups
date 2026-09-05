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

/// RFC 1157 error-status values.
const NO_SUCH_NAME: u32 = 2;
const GEN_ERR: u32 = 5;
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

        // Answer only for the OID we actually serve. The previous version
        // logged the requested OID and then returned the Brother status blob
        // for every request, so a walk or a query for sysDescr got printer
        // status as its answer.
        let Some(requested) = pdu.variable_bindings.first().map(|vb| vb.name.clone()) else {
            debug!(src = %src, "GET without a variable binding");
            respond_error(&socket, src, &msg.community, &pdu, GEN_ERR, 0).await;
            continue;
        };

        if requested != brother_oid {
            debug!(src = %src, oid = ?requested, "OID not served");
            respond_error(&socket, src, &msg.community, &pdu, NO_SUCH_NAME, 1).await;
            continue;
        }

        // try_lock rather than lock: a status request injected between two
        // chunks of a print job corrupts the raster stream, so a busy printer
        // is reported as busy instead of being interrupted.
        let status_bytes = match printer.try_lock() {
            Ok(guard) => match *guard {
                Some(ref p) => p.read().await.ok().map(|s| s.to_vec()),
                None => None,
            },
            Err(_) => None,
        };

        let Some(status_bytes) = status_bytes else {
            // A silent drop makes the client wait for its timeout. genErr
            // says "asked the right thing, cannot answer right now".
            debug!(src = %src, "printer busy or absent");
            respond_error(&socket, src, &msg.community, &pdu, GEN_ERR, 1).await;
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

/// Reply with an `SNMPv1` error status instead of staying silent.
///
/// RFC 1157 wants the response to echo the request's variable bindings, so the
/// manager can match the error to what it asked for.
async fn respond_error(
    socket: &UdpSocket,
    src: std::net::SocketAddr,
    community: &OctetString,
    pdu: &Pdu,
    error_status: u32,
    error_index: u32,
) {
    let response = Message {
        version: Integer::from(0),
        community: community.clone(),
        data: GetResponse(Pdu {
            request_id: pdu.request_id.clone(),
            error_status: Integer::from(error_status),
            error_index: Integer::from(error_index),
            variable_bindings: pdu.variable_bindings.clone(),
        }),
    };
    if let Ok(encoded) = rasn::ber::encode(&response) {
        let _ = socket.send_to(&encoded, src).await;
    }
}
