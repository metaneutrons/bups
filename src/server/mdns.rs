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
use crate::usb::Device;

/// Advertise printer via mDNS.
pub fn advertise(device: Device, serial: &str, port: u16, hostname: Option<&str>) -> Option<ServiceDaemon> {
    let mdns = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            error!(error = %e, "failed to create mDNS daemon");
            return None;
        }
    };

    let local_ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string());

    let model = device.model_name();
    let hostname = hostname
        .map(|h| h.to_string())
        .unwrap_or_else(|| format!("BRN{}", serial.replace(['-', ':'], "").to_uppercase()));
    let service_name = format!("bups {}", model);
    let caps = device.capabilities();
    let tf = |b: bool| if b { "T" } else { "F" };

    let product = format!("({})", model);
    let adminurl = format!("http://{}.local./", hostname);

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
        ("Color", tf(caps.color)),
        ("Copies", tf(caps.copies)),
        ("Duplex", tf(caps.duplex)),
        ("PaperCustom", tf(caps.paper_custom)),
        ("Binary", tf(caps.binary)),
        ("Transparent", tf(caps.transparent)),
        ("TBCP", tf(caps.tbcp)),
    ];

    let service = match ServiceInfo::new(
        MDNS_SERVICE_TYPE,
        &service_name,
        &format!("{}.local.", hostname),
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
