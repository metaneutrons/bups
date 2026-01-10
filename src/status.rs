// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! Printer status parsing.
//!
//! Parses 32-byte status responses from Brother printers.
//! Status format documented in Brother Raster Command Reference.

use crate::config::USB_READ_SIZE;

/// Status response magic bytes.
const STATUS_MAGIC: [u8; 2] = [0x80, 0x20];

/// Parsed printer status (32-byte response).
#[derive(Clone, Debug)]
#[allow(dead_code)] // Fields used for Debug output
pub struct Status {
    pub error1: u8,
    pub error2: u8,
    pub media_width: u8,
    pub media_type: u8,
    pub status_type: u8,
    pub phase: u8,
    pub tape_color: u8,
    pub text_color: u8,
}

impl Status {
    /// Parse and validate status from raw bytes.
    pub fn parse(raw: [u8; USB_READ_SIZE]) -> Option<Self> {
        if raw[0..2] != STATUS_MAGIC {
            return None;
        }
        Some(Self {
            error1: raw[8],
            error2: raw[9],
            media_width: raw[10],
            media_type: raw[11],
            status_type: raw[18],
            phase: raw[20],
            tape_color: raw[24],
            text_color: raw[25],
        })
    }

    /// Get error message if any.
    pub fn error_message(&self) -> Option<&'static str> {
        if self.error1 & 0x01 != 0 { return Some("No media"); }
        if self.error1 & 0x04 != 0 { return Some("Cutter jam"); }
        if self.error1 & 0x08 != 0 { return Some("Weak battery"); }
        if self.error1 & 0x40 != 0 { return Some("High voltage"); }
        if self.error2 & 0x01 != 0 { return Some("Wrong media"); }
        if self.error2 & 0x10 != 0 { return Some("Cover open"); }
        if self.error2 & 0x20 != 0 { return Some("Overheating"); }
        None
    }
}
