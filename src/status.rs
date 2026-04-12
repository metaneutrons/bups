// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! Printer status parsing.
//!
//! Parses 32-byte status responses from Brother printers.
//! Format documented in the Brother Raster Command Reference.

use crate::config::{
    ERR1_CUTTER_JAM, ERR1_HIGH_VOLTAGE, ERR1_NO_MEDIA, ERR1_WEAK_BATTERY, ERR2_COVER_OPEN,
    ERR2_OVERHEATING, ERR2_WRONG_MEDIA, STATUS_MAGIC, STATUS_OFF_ERROR1, STATUS_OFF_ERROR2,
    STATUS_OFF_MEDIA_TYPE, STATUS_OFF_MEDIA_WIDTH, STATUS_OFF_PHASE, STATUS_OFF_STATUS_TYPE,
    STATUS_OFF_TAPE_COLOR, STATUS_OFF_TEXT_COLOR, USB_READ_SIZE,
};

/// Parsed printer status from a 32-byte response.
///
/// All fields are parsed from the Brother protocol. Some are not yet
/// consumed but are part of the public API for downstream use.
#[derive(Clone, Debug)]
pub struct Status {
    pub error1: u8,
    pub error2: u8,
    pub media_width: u8,
    #[expect(dead_code, reason = "parsed from protocol, reserved for future use")]
    pub media_type: u8,
    #[expect(dead_code, reason = "parsed from protocol, reserved for future use")]
    pub kind: u8,
    #[expect(dead_code, reason = "parsed from protocol, reserved for future use")]
    pub phase: u8,
    #[expect(dead_code, reason = "parsed from protocol, reserved for future use")]
    pub tape_color: u8,
    #[expect(dead_code, reason = "parsed from protocol, reserved for future use")]
    pub text_color: u8,
}

impl Status {
    /// Parse and validate status from raw bytes.
    ///
    /// Returns `None` if the magic header bytes don't match.
    #[must_use]
    pub fn parse(raw: [u8; USB_READ_SIZE]) -> Option<Self> {
        if raw[0..2] != STATUS_MAGIC {
            return None;
        }
        Some(Self {
            error1: raw[STATUS_OFF_ERROR1],
            error2: raw[STATUS_OFF_ERROR2],
            media_width: raw[STATUS_OFF_MEDIA_WIDTH],
            media_type: raw[STATUS_OFF_MEDIA_TYPE],
            kind: raw[STATUS_OFF_STATUS_TYPE],
            phase: raw[STATUS_OFF_PHASE],
            tape_color: raw[STATUS_OFF_TAPE_COLOR],
            text_color: raw[STATUS_OFF_TEXT_COLOR],
        })
    }

    /// Collect all active error conditions.
    ///
    /// Returns an empty vec when the printer is healthy.
    #[must_use]
    pub fn errors(&self) -> Vec<&'static str> {
        let checks: &[(u8, u8, &str)] = &[
            (self.error1, ERR1_NO_MEDIA, "No media"),
            (self.error1, ERR1_CUTTER_JAM, "Cutter jam"),
            (self.error1, ERR1_WEAK_BATTERY, "Weak battery"),
            (self.error1, ERR1_HIGH_VOLTAGE, "High voltage"),
            (self.error2, ERR2_WRONG_MEDIA, "Wrong media"),
            (self.error2, ERR2_COVER_OPEN, "Cover open"),
            (self.error2, ERR2_OVERHEATING, "Overheating"),
        ];
        checks
            .iter()
            .filter(|(byte, flag, _)| byte & flag != 0)
            .map(|(_, _, msg)| *msg)
            .collect()
    }

    /// `true` when any error flag is set.
    #[must_use]
    pub const fn has_error(&self) -> bool {
        self.error1 != 0 || self.error2 != 0
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let errors = self.errors();
        if errors.is_empty() {
            write!(f, "READY (media {}mm)", self.media_width)
        } else {
            write!(f, "ERROR: {}", errors.join(", "))
        }
    }
}
