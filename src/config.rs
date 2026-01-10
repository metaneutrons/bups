// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! Configuration constants.

// USB
pub const BROTHER_VENDOR_ID: u16 = 0x04f9;
pub const USB_TIMEOUT_MS: u64 = 2000; // QL printers need longer timeouts

// Device init commands
pub const INVALIDATE_CMD: [u8; 200] = [0x00; 200];
pub const INIT_CMD: [u8; 2] = [0x1b, 0x40]; // ESC @

// Network ports
pub const TCP_PORT: u16 = 9100;
pub const SNMP_PORT: u16 = 161;

// Buffer sizes
pub const TCP_BUFFER_SIZE: usize = 8192;
pub const USB_READ_SIZE: usize = 32;

// Timing
pub const POST_WRITE_DELAY_MS: u64 = 100;

// Reconnect defaults
pub const DEFAULT_RECONNECT_INTERVAL: u64 = 30;
pub const DEFAULT_MAX_RECONNECT_ATTEMPTS: u32 = 0; // 0 = infinite

// mDNS
pub const MDNS_SERVICE_TYPE: &str = "_pdl-datastream._tcp.local.";
pub const BROTHER_PDL: &str = "application/vnd.brother-hbp";

// SNMP - Brother status OID: 1.3.6.1.4.1.2435.3.3.9.1.6.1.0
pub const BROTHER_STATUS_OID: &[u32] = &[1, 3, 6, 1, 4, 1, 2435, 3, 3, 9, 1, 6, 1, 0];
