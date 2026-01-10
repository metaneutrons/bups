// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! Printer status parsing.
//!
//! Parses 32-byte status responses from Brother printers.
//! Status format documented in Brother Raster Command Reference.

use crate::config::{
    ERR1_CUTTER_JAM, ERR1_HIGH_VOLTAGE, ERR1_NO_MEDIA, ERR1_WEAK_BATTERY, ERR2_COVER_OPEN,
    ERR2_OVERHEATING, ERR2_WRONG_MEDIA, STATUS_MAGIC, STATUS_OFF_ERROR1, STATUS_OFF_ERROR2,
    STATUS_OFF_MEDIA_TYPE, STATUS_OFF_MEDIA_WIDTH, STATUS_OFF_PHASE, STATUS_OFF_STATUS_TYPE,
    STATUS_OFF_TAPE_COLOR, STATUS_OFF_TEXT_COLOR, USB_READ_SIZE,
};

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
            error1: raw[STATUS_OFF_ERROR1],
            error2: raw[STATUS_OFF_ERROR2],
            media_width: raw[STATUS_OFF_MEDIA_WIDTH],
            media_type: raw[STATUS_OFF_MEDIA_TYPE],
            status_type: raw[STATUS_OFF_STATUS_TYPE],
            phase: raw[STATUS_OFF_PHASE],
            tape_color: raw[STATUS_OFF_TAPE_COLOR],
            text_color: raw[STATUS_OFF_TEXT_COLOR],
        })
    }

    /// Get error message if any.
    pub fn error_message(&self) -> Option<&'static str> {
        if self.error1 & ERR1_NO_MEDIA != 0 {
            return Some("No media");
        }
        if self.error1 & ERR1_CUTTER_JAM != 0 {
            return Some("Cutter jam");
        }
        if self.error1 & ERR1_WEAK_BATTERY != 0 {
            return Some("Weak battery");
        }
        if self.error1 & ERR1_HIGH_VOLTAGE != 0 {
            return Some("High voltage");
        }
        if self.error2 & ERR2_WRONG_MEDIA != 0 {
            return Some("Wrong media");
        }
        if self.error2 & ERR2_COVER_OPEN != 0 {
            return Some("Cover open");
        }
        if self.error2 & ERR2_OVERHEATING != 0 {
            return Some("Overheating");
        }
        None
    }
}
