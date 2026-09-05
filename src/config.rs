// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! Configuration constants (single source of truth).

// --- USB ---

pub const BROTHER_VENDOR_ID: u16 = 0x04f9;
/// The printer-class interface. Every model in the table exposes it as 0;
/// an untested model that does not is the likely cause of an open failure.
pub const USB_INTERFACE: u8 = 0;
pub const USB_TIMEOUT_MS: u64 = 2000;
pub const USB_EP_OUT: u8 = 0x02;
pub const USB_EP_IN: u8 = 0x81;
pub const USB_READ_SIZE: usize = 32;
pub const USB_READ_BUFFER: usize = 64;
pub const USB_READ_RETRIES: u32 = 10;
pub const USB_READ_TIMEOUT_MS: u64 = 500;
pub const USB_STATUS_DELAY_MS: u64 = 50;
pub const USB_WRITE_CHUNK: usize = 4096;

// --- Device init commands ---

pub const INVALIDATE_CMD: [u8; 200] = [0x00; 200];
pub const INIT_CMD: [u8; 2] = [0x1b, 0x40]; // ESC @
pub const STATUS_REQUEST: [u8; 3] = [0x1b, 0x69, 0x53]; // ESC i S

// --- Status response format (32-byte) ---
//
// Offsets from the manufacturer references, which agree on every field below:
//   PT: Raster Command Reference PT-E550W/P750W/P710BT, rev. 1.02, section 4
//   QL: Raster Command Reference QL-710W/720NW, rev. 1.0, section 4
// The *meanings* of media type and the error bits differ between the two;
// those tables live in `crate::status` because they are series-specific.

/// Bytes 0 and 1: print head mark 80h, size 20h.
pub const STATUS_MAGIC: [u8; 2] = [0x80, 0x20];

pub const STATUS_OFF_ERROR1: usize = 8;
pub const STATUS_OFF_ERROR2: usize = 9;
pub const STATUS_OFF_MEDIA_WIDTH: usize = 10;
pub const STATUS_OFF_MEDIA_TYPE: usize = 11;
/// On QL die-cut labels this carries the die-cut length, which is what
/// identifies the label. Fixed at 00h for continuous tape and on PT.
pub const STATUS_OFF_MEDIA_LENGTH: usize = 17;
pub const STATUS_OFF_STATUS_TYPE: usize = 18;
/// Byte 19 is the phase *type*. Bytes 20 and 21 are the phase *number*,
/// big-endian; reading byte 20 as the phase yields the number's high byte,
/// which is 00h in every documented case.
pub const STATUS_OFF_PHASE_TYPE: usize = 19;
pub const STATUS_OFF_PHASE_NUMBER_HI: usize = 20;
pub const STATUS_OFF_PHASE_NUMBER_LO: usize = 21;
/// PT only. Bytes 24 to 31 are reserved and fixed at 00h on QL.
pub const STATUS_OFF_TAPE_COLOR: usize = 24;
/// PT only, see [`STATUS_OFF_TAPE_COLOR`].
pub const STATUS_OFF_TEXT_COLOR: usize = 25;

// --- Network ---

pub const TCP_PORT: u16 = 9100;
pub const SNMP_PORT: u16 = 161;
pub const TCP_BUFFER_SIZE: usize = 8192;
pub const SNMP_BUFFER_SIZE: usize = 1024;
pub const POST_WRITE_DELAY_MS: u64 = 100;

// --- Reconnect ---

pub const DEFAULT_RECONNECT_INTERVAL: u64 = 30;
pub const DEFAULT_MAX_RECONNECT_ATTEMPTS: u32 = 0; // 0 = infinite

// --- mDNS ---

pub const MDNS_SERVICE_TYPE: &str = "_pdl-datastream._tcp.local.";
pub const BROTHER_PDL: &str = "application/vnd.brother-hbp";

// --- SNMP ---

/// Brother status OID: `1.3.6.1.4.1.2435.3.3.9.1.6.1.0`
pub const BROTHER_STATUS_OID: &[u32] = &[1, 3, 6, 1, 4, 1, 2435, 3, 3, 9, 1, 6, 1, 0];
