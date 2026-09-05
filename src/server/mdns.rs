// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! mDNS/Bonjour service advertisement.
//!
//! Advertises the printer as `_pdl-datastream._tcp` for automatic discovery
//! by macOS, iOS, and other Bonjour-aware clients.

use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceInfo};
use tracing::{error, info, warn};

use crate::config::{BROTHER_PDL, MDNS_RETIRE_TIMEOUT_MS, MDNS_SERVICE_TYPE};
use crate::usb::Device;
use crate::usb::device::Capabilities;

/// A live mDNS advertisement.
///
/// # Why this is not a bare `ServiceDaemon`
///
/// `ServiceDaemon` is `#[derive(Clone)]`, a handle onto a background thread,
/// and it has no `Drop` implementation. Dropping one drops a handle and
/// nothing more: the thread keeps running and the service stays advertised.
/// This module used to claim the opposite and act on it, so a printer that
/// went away kept being announced and every reconnect started another daemon
/// beside the last.
///
/// Measured against a real network with `dns-sd -B _pdl-datastream._tcp
/// local.`: after the handle was dropped, no removal followed within fourteen
/// seconds, under mdns-sd 0.19.2 and 0.21.1 alike.
///
/// Retiring an advertisement is therefore something the caller has to say.
pub struct Advertisement {
    /// `None` once retired. Only so the `Drop` backstop can tell whether the
    /// daemon has already been asked to stop.
    daemon: Option<ServiceDaemon>,
    /// Carried for the log line, nothing else. `shutdown` needs no name.
    name: String,
}

impl Advertisement {
    /// Send the goodbye packets and stop the daemon thread.
    ///
    /// `shutdown` alone is enough. The daemon answers the exit command by
    /// unregistering every service it holds, goodbye packets included, before
    /// it stops; a separate `unregister` call would be the same work twice.
    ///
    /// The wait is not politeness. The service name is derived from the model,
    /// so a printer that reconnects is announced under the name it had before.
    /// If the goodbye left after that second registration, a browser would
    /// apply it to the new record and drop a printer that is in fact present.
    pub async fn retire(mut self) {
        let Some(daemon) = self.daemon.take() else {
            return;
        };
        let stopped = match daemon.shutdown() {
            Ok(rx) => rx,
            Err(e) => {
                warn!(error = %e, name = %self.name, "mDNS shutdown could not be requested");
                return;
            }
        };
        let limit = Duration::from_millis(MDNS_RETIRE_TIMEOUT_MS);
        match tokio::time::timeout(limit, stopped.recv_async()).await {
            Ok(Ok(_)) => info!(name = %self.name, "mDNS retired"),
            Ok(Err(e)) => warn!(error = %e, name = %self.name, "mDNS daemon gave no answer"),
            Err(_) => warn!(
                name = %self.name,
                ms = MDNS_RETIRE_TIMEOUT_MS,
                "mDNS daemon did not confirm the shutdown in time"
            ),
        }
    }
}

impl Drop for Advertisement {
    fn drop(&mut self) {
        // A backstop, not the normal path: `retire` has taken the daemon by
        // the time this runs. If it has not -- a panic, or an early return
        // added later -- the daemon is still asked to send its goodbyes and
        // stop, because dropping the handle alone would leave the thread
        // running and the printer announced.
        //
        // It cannot wait for the answer. Drop is not async, and blocking here
        // would block a tokio worker thread.
        if let Some(daemon) = self.daemon.take() {
            warn!(name = %self.name, "mDNS advertisement dropped without retire");
            let _ = daemon.shutdown();
        }
    }
}

/// Advertise a printer via mDNS.
///
/// The advertisement stays up until [`Advertisement::retire`] is called.
pub fn advertise(
    device: Device,
    serial: &str,
    port: u16,
    hostname: Option<&str>,
) -> Option<Advertisement> {
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

    Some(Advertisement {
        daemon: Some(mdns),
        name: service_name,
    })
}

/// Run the mDNS re-advertisement loop.
///
/// Re-advertises whenever the printer changes (connect / disconnect).
pub async fn mdns_loop(
    mut rx: tokio::sync::watch::Receiver<Option<(Device, String)>>,
    port: u16,
    hostname: Option<String>,
) {
    let mut live: Option<Advertisement> = None;
    let mut current: Option<(Device, String)> = None;

    // Handle initial value.
    {
        let value = rx.borrow_and_update().clone();
        if let Some((device, ref serial)) = value {
            live = advertise(device, serial, port, hostname.as_deref());
            current = value;
        }
    }

    while rx.changed().await.is_ok() {
        let value = rx.borrow().clone();
        if value == current {
            continue;
        }
        current.clone_from(&value);

        // Retire the old advertisement before announcing the new one. Awaited
        // rather than dropped: an assignment here would only drop a handle,
        // leave the previous printer announced and the previous daemon thread
        // running, which is what this loop did until now.
        if let Some(old) = live.take() {
            old.retire().await;
        }
        if let Some((device, ref serial)) = value {
            live = advertise(device, serial, port, hostname.as_deref());
        }
    }

    // The sender is gone, so the process is on its way out. Say goodbye while
    // there is still a runtime to await on; the Drop backstop could only fire
    // the command and never see it answered.
    if let Some(last) = live.take() {
        last.retire().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What this covers and what it does not.
    ///
    /// Retiring twice must not panic, and the `Drop` backstop must stay quiet
    /// afterwards. That is bookkeeping, and it is the part a later edit is
    /// most likely to get wrong.
    ///
    /// It says nothing about the network. Proving that a goodbye actually
    /// leaves the host needs a daemon, a multicast interface and a browser,
    /// none of which a CI runner is guaranteed to have, and a flaky test here
    /// would be worse than an honest gap. That half was measured by hand
    /// against a real LAN; the pull request carries the `dns-sd` output.
    #[tokio::test]
    async fn retiring_a_retired_advertisement_does_nothing() {
        let retired = Advertisement {
            daemon: None,
            name: "bups PT-P900".to_owned(),
        };
        retired.retire().await;
    }
}
