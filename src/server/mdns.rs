// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! mDNS/Bonjour service advertisement.
//!
//! Advertises the printer as `_pdl-datastream._tcp` for automatic discovery
//! by macOS, iOS, and other Bonjour-aware clients.

use mdns_sd::{ServiceDaemon, ServiceInfo};
use tracing::{error, info};

use crate::config::{BROTHER_PDL, MDNS_SERVICE_TYPE};
use crate::usb::Device;
use crate::usb::device::Capabilities;

/// Advertise a printer via mDNS. Returns the daemon handle (dropping it
/// unregisters the service).
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
        local_ip_address::local_ip().map_or_else(|_| "127.0.0.1".to_owned(), |ip| ip.to_string());

    let model = device.model_name();
    let hostname = hostname.map_or_else(
        || format!("BRN{}", serial.replace(['-', ':'], "").to_uppercase()),
        ToString::to_string,
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
        Ok(()) => {
            info!(
                name = %service_name,
                ip = %local_ip,
                port,
                "mDNS registered"
            );
        }
        Err(e) => error!(error = %e, "mDNS registration failed"),
    }

    Some(mdns)
}

/// Run the mDNS re-advertisement loop.
///
/// Re-advertises whenever the printer changes (connect / disconnect).
pub async fn mdns_loop(
    mut rx: tokio::sync::watch::Receiver<Option<(Device, String)>>,
    port: u16,
    hostname: Option<String>,
) {
    // The daemon handle must be kept alive — dropping it unregisters the service.
    #[allow(clippy::collection_is_never_read)]
    let mut _mdns: Option<ServiceDaemon> = None;
    let mut current: Option<(Device, String)> = None;

    // Handle initial value.
    {
        let value = rx.borrow_and_update().clone();
        if let Some((device, ref serial)) = value {
            _mdns = advertise(device, serial, port, hostname.as_deref());
            current = value;
        }
    }

    while rx.changed().await.is_ok() {
        let value = rx.borrow().clone();
        if value == current {
            continue;
        }
        current.clone_from(&value);

        // Drop old advertisement, create new one if printer connected.
        _mdns = None;
        if let Some((device, ref serial)) = value {
            _mdns = advertise(device, serial, port, hostname.as_deref());
        }
    }
}
