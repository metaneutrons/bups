// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! mDNS/Bonjour service advertisement.
//!
//! Advertises printer as `_pdl-datastream._tcp` for automatic discovery
//! by macOS, iOS, and other Bonjour-aware clients.

use mdns_sd::{ServiceDaemon, ServiceInfo};
use tracing::{error, info};

use crate::config::{BROTHER_PDL, MDNS_SERVICE_TYPE};
use crate::usb::device::Capabilities;
use crate::usb::Device;

/// Advertise printer via mDNS.
pub fn advertise(
    device: Device,
    serial: &str,
    port: u16,
    hostname: Option<&str>,
) -> Option<ServiceDaemon> {
    let mdns = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            error!(error = %e, "failed to create mDNS daemon");
            return None;
        }
    };

    let local_ip =
        local_ip_address::local_ip().map_or_else(|_| "127.0.0.1".to_string(), |ip| ip.to_string());

    let model = device.model_name();
    let hostname = hostname.map_or_else(
        || format!("BRN{}", serial.replace(['-', ':'], "").to_uppercase()),
        |h| h.to_string(),
    );
    let service_name = format!("bups {model}");
    let caps = device.capabilities();
    let tf = |c: Capabilities| if caps.contains(c) { "T" } else { "F" };

    let product = format!("({model})");
    let adminurl = format!("http://{hostname}.local./");

    let props = [
        ("txtvers", "1"),
        ("qtotal", "1"),
        ("pdl", BROTHER_PDL),
        ("ty", model),
        ("product", product.as_str()),
        ("adminurl", adminurl.as_str()),
        ("priority", "25"),
        ("usb_MFG", "Brother"),
        ("usb_MDL", model),
        ("Color", tf(Capabilities::COLOR)),
        ("Copies", tf(Capabilities::COPIES)),
        ("Duplex", tf(Capabilities::DUPLEX)),
        ("PaperCustom", tf(Capabilities::PAPER_CUSTOM)),
        ("Binary", tf(Capabilities::BINARY)),
        ("Transparent", tf(Capabilities::TRANSPARENT)),
        ("TBCP", tf(Capabilities::TBCP)),
    ];

    let service = match ServiceInfo::new(
        MDNS_SERVICE_TYPE,
        &service_name,
        &format!("{hostname}.local."),
        &local_ip,
        port,
        &props[..],
    ) {
        Ok(s) => s,
        Err(e) => {
            error!(error = %e, "failed to create service info");
            return None;
        }
    };

    match mdns.register(service) {
        Ok(_) => info!(name = %service_name, ip = %local_ip, port, "mDNS registered"),
        Err(e) => error!(error = %e, "mDNS registration failed"),
    }

    Some(mdns)
}
