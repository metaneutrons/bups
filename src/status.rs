// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! Printer status parsing.
//!
//! Parses the 32-byte status response Brother printers return to `ESC i S`.
//!
//! Both supported families use the same frame, but they are documented in two
//! different manuals and several fields do **not** mean the same thing:
//!
//! - the media type table shares only the value `0x00`
//! - error bits overlap on three of sixteen and otherwise differ entirely
//! - tape and text colour exist on PT only; on QL those bytes are reserved
//!
//! Parsing therefore takes the [`Series`] and dispatches on it. Offsets and
//! values below are taken from the manufacturer references, not from another
//! implementation:
//!
//! - PT: Raster Command Reference PT-E550W/P750W/P710BT, rev. 1.02, section 4
//! - QL: Raster Command Reference QL-710W/720NW, rev. 1.0, section 4

use crate::config::{
    STATUS_MAGIC, STATUS_OFF_ERROR1, STATUS_OFF_ERROR2, STATUS_OFF_MEDIA_LENGTH,
    STATUS_OFF_MEDIA_TYPE, STATUS_OFF_MEDIA_WIDTH, STATUS_OFF_PHASE_NUMBER_HI,
    STATUS_OFF_PHASE_NUMBER_LO, STATUS_OFF_PHASE_TYPE, STATUS_OFF_STATUS_TYPE,
    STATUS_OFF_TAPE_COLOR, STATUS_OFF_TEXT_COLOR, USB_READ_SIZE,
};
use crate::usb::device::Series;

// ---------------------------------------------------------------------------
// Status type, shared
// ---------------------------------------------------------------------------

/// Why the printer sent this frame. Table (5) in both references.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusType {
    ReplyToRequest,
    PrintingCompleted,
    ErrorOccurred,
    /// PT only, and marked "not used" even there.
    ExitIfMode,
    TurnedOff,
    Notification,
    PhaseChange,
    Unknown(u8),
}

impl From<u8> for StatusType {
    fn from(v: u8) -> Self {
        match v {
            0x00 => Self::ReplyToRequest,
            0x01 => Self::PrintingCompleted,
            0x02 => Self::ErrorOccurred,
            0x03 => Self::ExitIfMode,
            0x04 => Self::TurnedOff,
            0x05 => Self::Notification,
            0x06 => Self::PhaseChange,
            other => Self::Unknown(other),
        }
    }
}

// ---------------------------------------------------------------------------
// Phase, shared
// ---------------------------------------------------------------------------

/// Table (6) in both references. The phase *number* is a separate `u16`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseType {
    Editing,
    Printing,
    Unknown(u8),
}

impl From<u8> for PhaseType {
    fn from(v: u8) -> Self {
        match v {
            0x00 => Self::Editing,
            0x01 => Self::Printing,
            other => Self::Unknown(other),
        }
    }
}

// ---------------------------------------------------------------------------
// Media type, per series
// ---------------------------------------------------------------------------

/// Table (4). The two families share only `0x00`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaType {
    None,
    // PT
    LaminatedTape,
    NonLaminatedTape,
    HeatShrinkTube21,
    HeatShrinkTube31,
    IncompatibleTape,
    // QL
    ContinuousLength,
    DieCutLabels,
    Unknown(u8),
}

impl MediaType {
    const fn parse(series: Series, v: u8) -> Self {
        match (series, v) {
            (_, 0x00) => Self::None,
            (Series::Pt, 0x01) => Self::LaminatedTape,
            (Series::Pt, 0x03) => Self::NonLaminatedTape,
            (Series::Pt, 0x11) => Self::HeatShrinkTube21,
            (Series::Pt, 0x17) => Self::HeatShrinkTube31,
            (Series::Pt, 0xFF) => Self::IncompatibleTape,
            (Series::Ql, 0x4A) => Self::ContinuousLength,
            (Series::Ql, 0x4B) => Self::DieCutLabels,
            (_, other) => Self::Unknown(other),
        }
    }

    /// Human-readable name for the `STATUS` command and logs.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "no media",
            Self::LaminatedTape => "laminated tape",
            Self::NonLaminatedTape => "non-laminated tape",
            Self::HeatShrinkTube21 => "heat-shrink tube 2:1",
            Self::HeatShrinkTube31 => "heat-shrink tube 3:1",
            Self::IncompatibleTape => "incompatible tape",
            Self::ContinuousLength => "continuous length tape",
            Self::DieCutLabels => "die-cut labels",
            Self::Unknown(_) => "unknown",
        }
    }
}

// ---------------------------------------------------------------------------
// Error bits, per series
// ---------------------------------------------------------------------------

/// Tables (1) and (2). Entries the reference marks "not used" are absent, so
/// an unmatched bit shows up as a raw value rather than a wrong label.
const PT_ERROR1: &[(u8, &str)] = &[
    (0x01, "no media"),
    (0x04, "cutter jam"),
    (0x08, "weak batteries"),
    (0x40, "high-voltage adapter"),
];

const PT_ERROR2: &[(u8, &str)] = &[
    (0x01, "wrong media"),
    (0x10, "cover open"),
    (0x20, "overheating"),
];

const QL_ERROR1: &[(u8, &str)] = &[
    (0x01, "no media"),
    (0x02, "end of media"),
    (0x04, "cutter jam"),
    (0x10, "printer in use"),
    (0x20, "printer turned off"),
];

const QL_ERROR2: &[(u8, &str)] = &[
    (0x01, "replace media"),
    (0x02, "expansion buffer full"),
    (0x04, "communication error"),
    (0x10, "cover open"),
    (0x40, "media cannot be fed"),
    (0x80, "system error"),
];

/// One entry per documented error bit: mask and the name it stands for.
type ErrorTable = &'static [(u8, &'static str)];

const fn error_tables(series: Series) -> (ErrorTable, ErrorTable) {
    match series {
        Series::Pt => (PT_ERROR1, PT_ERROR2),
        Series::Ql => (QL_ERROR1, QL_ERROR2),
    }
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

/// A parsed 32-byte status frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub series: Series,
    pub error1: u8,
    pub error2: u8,
    /// Millimetres. On QL die-cut labels this is the width of the die-cut area.
    pub media_width: u8,
    /// Millimetres. Fixed at 0 for continuous tape; on QL die-cut labels this
    /// is the length of the die-cut area, which is what identifies the label.
    pub media_length: u8,
    pub media_type: MediaType,
    /// Why the printer sent this frame.
    pub kind: StatusType,
    pub phase_type: PhaseType,
    pub phase_number: u16,
    /// PT only. Bytes 24 and 25 are reserved on QL.
    pub tape_color: Option<u8>,
    /// PT only.
    pub text_color: Option<u8>,
}

impl Status {
    /// Parse a status frame. Returns `None` when the magic header is absent.
    #[must_use]
    pub fn parse(raw: [u8; USB_READ_SIZE], series: Series) -> Option<Self> {
        if raw[0..2] != STATUS_MAGIC {
            return None;
        }
        let (tape_color, text_color) = match series {
            Series::Pt => (
                Some(raw[STATUS_OFF_TAPE_COLOR]),
                Some(raw[STATUS_OFF_TEXT_COLOR]),
            ),
            Series::Ql => (None, None),
        };
        Some(Self {
            series,
            error1: raw[STATUS_OFF_ERROR1],
            error2: raw[STATUS_OFF_ERROR2],
            media_width: raw[STATUS_OFF_MEDIA_WIDTH],
            media_length: raw[STATUS_OFF_MEDIA_LENGTH],
            media_type: MediaType::parse(series, raw[STATUS_OFF_MEDIA_TYPE]),
            kind: StatusType::from(raw[STATUS_OFF_STATUS_TYPE]),
            phase_type: PhaseType::from(raw[STATUS_OFF_PHASE_TYPE]),
            phase_number: u16::from_be_bytes([
                raw[STATUS_OFF_PHASE_NUMBER_HI],
                raw[STATUS_OFF_PHASE_NUMBER_LO],
            ]),
            tape_color,
            text_color,
        })
    }

    /// Active error conditions, named per series.
    ///
    /// A bit the reference marks "not used" has no name, so it is reported as
    /// its raw value instead of being silently dropped. That matters: the old
    /// parser knew only the PT names, so an out-of-labels QL reported an error
    /// with an empty cause list.
    #[must_use]
    pub fn errors(&self) -> Vec<String> {
        let (t1, t2) = error_tables(self.series);
        let mut out = Vec::new();
        let mut named1 = 0u8;
        let mut named2 = 0u8;
        for (mask, name) in t1 {
            if self.error1 & mask != 0 {
                out.push((*name).to_owned());
                named1 |= mask;
            }
        }
        for (mask, name) in t2 {
            if self.error2 & mask != 0 {
                out.push((*name).to_owned());
                named2 |= mask;
            }
        }
        let rest1 = self.error1 & !named1;
        let rest2 = self.error2 & !named2;
        if rest1 != 0 {
            out.push(format!("undocumented error1 bits {rest1:#04x}"));
        }
        if rest2 != 0 {
            out.push(format!("undocumented error2 bits {rest2:#04x}"));
        }
        out
    }

    /// `true` when any error bit is set, named or not.
    #[must_use]
    pub const fn has_error(&self) -> bool {
        self.error1 != 0 || self.error2 != 0
    }

    /// Media description for the `STATUS` command.
    #[must_use]
    pub fn media_description(&self) -> String {
        let kind = self.media_type.label();
        if self.media_length == 0 {
            format!("{kind}, {}mm", self.media_width)
        } else {
            format!("{kind}, {}x{}mm", self.media_width, self.media_length)
        }
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let errors = self.errors();
        if errors.is_empty() {
            write!(f, "READY ({})", self.media_description())
        } else {
            write!(f, "ERROR: {}", errors.join(", "))
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "panicking is how a test reports failure"
)]
mod tests {
    use super::*;

    /// A frame with the mandatory header bytes and everything else zeroed.
    fn frame() -> [u8; USB_READ_SIZE] {
        let mut f = [0u8; USB_READ_SIZE];
        f[0] = 0x80; // print head mark
        f[1] = 0x20; // size
        f[2] = b'B';
        f[3] = b'0';
        f
    }

    #[test]
    fn rejects_a_frame_without_the_magic_header() {
        let mut f = frame();
        f[0] = 0x00;
        assert!(Status::parse(f, Series::Pt).is_none());
        let mut f = frame();
        f[1] = 0x21;
        assert!(Status::parse(f, Series::Ql).is_none());
    }

    #[test]
    fn accepts_a_bare_valid_frame() {
        assert!(Status::parse(frame(), Series::Pt).is_some());
        assert!(Status::parse(frame(), Series::Ql).is_some());
    }

    /// Offsets against the manufacturer references, one distinct value per
    /// field so a transposition cannot pass.
    #[test]
    fn reads_every_field_from_its_documented_offset() {
        let mut f = frame();
        f[8] = 0x01; // error information 1
        f[9] = 0x10; // error information 2
        f[10] = 24; // media width
        f[11] = 0x01; // media type, PT laminated
        f[17] = 42; // media length
        f[18] = 0x02; // status type, error occurred
        f[19] = 0x01; // phase type, printing
        f[20] = 0x00; // phase number, high byte
        f[21] = 0x14; // phase number, low byte
        f[24] = 0x04; // tape colour
        f[25] = 0x08; // text colour

        let s = Status::parse(f, Series::Pt).expect("valid frame");
        assert_eq!(s.error1, 0x01);
        assert_eq!(s.error2, 0x10);
        assert_eq!(s.media_width, 24);
        assert_eq!(s.media_length, 42);
        assert_eq!(s.media_type, MediaType::LaminatedTape);
        assert_eq!(s.kind, StatusType::ErrorOccurred);
        assert_eq!(s.phase_type, PhaseType::Printing);
        assert_eq!(s.phase_number, 20);
        assert_eq!(s.tape_color, Some(0x04));
        assert_eq!(s.text_color, Some(0x08));
    }

    /// Byte 19 is the phase *type*; 20 and 21 are the phase number. The old
    /// parser read byte 20 and called it the phase, which is the high byte of
    /// a number that is 0 in every documented case.
    #[test]
    fn phase_type_and_phase_number_are_separate_fields() {
        let mut f = frame();
        f[19] = 0x01;
        f[20] = 0x00;
        f[21] = 0x0A;
        let s = Status::parse(f, Series::Pt).expect("valid frame");
        assert_eq!(s.phase_type, PhaseType::Printing);
        assert_eq!(s.phase_number, 10);
    }

    #[test]
    fn ql_leaves_the_reserved_colour_bytes_alone() {
        let mut f = frame();
        f[24] = 0xAB;
        f[25] = 0xCD;
        let s = Status::parse(f, Series::Ql).expect("valid frame");
        assert_eq!(s.tape_color, None);
        assert_eq!(s.text_color, None);
    }

    #[test]
    fn media_type_tables_differ_between_the_series() {
        let mut f = frame();

        f[11] = 0x01;
        assert_eq!(
            Status::parse(f, Series::Pt).unwrap().media_type,
            MediaType::LaminatedTape
        );
        // 0x01 is not in the QL table.
        assert_eq!(
            Status::parse(f, Series::Ql).unwrap().media_type,
            MediaType::Unknown(0x01)
        );

        f[11] = 0x4B;
        assert_eq!(
            Status::parse(f, Series::Ql).unwrap().media_type,
            MediaType::DieCutLabels
        );
        // 0x4B is not in the PT table.
        assert_eq!(
            Status::parse(f, Series::Pt).unwrap().media_type,
            MediaType::Unknown(0x4B)
        );

        // 0x00 is the one value both share.
        f[11] = 0x00;
        assert_eq!(
            Status::parse(f, Series::Pt).unwrap().media_type,
            MediaType::None
        );
        assert_eq!(
            Status::parse(f, Series::Ql).unwrap().media_type,
            MediaType::None
        );
    }

    #[test]
    fn pt_error_bits_match_the_pt_reference() {
        let cases: &[(usize, u8, &str)] = &[
            (8, 0x01, "no media"),
            (8, 0x04, "cutter jam"),
            (8, 0x08, "weak batteries"),
            (8, 0x40, "high-voltage adapter"),
            (9, 0x01, "wrong media"),
            (9, 0x10, "cover open"),
            (9, 0x20, "overheating"),
        ];
        for (offset, mask, name) in cases {
            let mut f = frame();
            f[*offset] = *mask;
            let s = Status::parse(f, Series::Pt).expect("valid frame");
            assert!(s.has_error(), "{name}: has_error");
            assert_eq!(s.errors(), vec![(*name).to_owned()], "{name}");
        }
    }

    #[test]
    fn ql_error_bits_match_the_ql_reference() {
        let cases: &[(usize, u8, &str)] = &[
            (8, 0x01, "no media"),
            (8, 0x02, "end of media"),
            (8, 0x04, "cutter jam"),
            (8, 0x10, "printer in use"),
            (8, 0x20, "printer turned off"),
            (9, 0x01, "replace media"),
            (9, 0x02, "expansion buffer full"),
            (9, 0x04, "communication error"),
            (9, 0x10, "cover open"),
            (9, 0x40, "media cannot be fed"),
            (9, 0x80, "system error"),
        ];
        for (offset, mask, name) in cases {
            let mut f = frame();
            f[*offset] = *mask;
            let s = Status::parse(f, Series::Ql).expect("valid frame");
            assert!(s.has_error(), "{name}: has_error");
            assert_eq!(s.errors(), vec![(*name).to_owned()], "{name}");
        }
    }

    /// The regression this whole split exists for. A QL reporting "printer
    /// turned off" sets error1 bit 5, which on PT is documented as not used.
    /// The single-table parser therefore said "error" and listed no cause.
    #[test]
    fn a_switched_off_ql_names_its_cause() {
        let mut f = frame();
        f[8] = 0x20;
        let s = Status::parse(f, Series::Ql).expect("valid frame");
        assert!(s.has_error());
        assert_eq!(s.errors(), vec!["printer turned off".to_owned()]);
        assert!(s.to_string().contains("printer turned off"));
    }

    /// An error bit with no documented meaning must still be reported.
    #[test]
    fn undocumented_bits_are_reported_rather_than_dropped() {
        let mut f = frame();
        f[8] = 0x80; // PT: "not used"
        let s = Status::parse(f, Series::Pt).expect("valid frame");
        assert!(s.has_error());
        assert_eq!(s.errors(), vec!["undocumented error1 bits 0x80".to_owned()]);
        assert!(
            !s.errors().is_empty(),
            "has_error must never imply an empty cause list"
        );
    }

    /// Whatever the bits are, the two must not disagree.
    #[test]
    fn has_error_and_errors_never_contradict_each_other() {
        for series in [Series::Pt, Series::Ql] {
            for e1 in 0u16..=255 {
                for e2 in [0u8, 0x01, 0x02, 0x04, 0x10, 0x20, 0x40, 0x80] {
                    let mut f = frame();
                    #[expect(clippy::cast_possible_truncation, reason = "loop bound is 255")]
                    let e1 = e1 as u8;
                    f[8] = e1;
                    f[9] = e2;
                    let s = Status::parse(f, series).expect("valid frame");
                    assert_eq!(
                        s.has_error(),
                        !s.errors().is_empty(),
                        "{series:?} error1={e1:#04x} error2={e2:#04x}"
                    );
                }
            }
        }
    }

    #[test]
    fn media_description_shows_length_only_when_it_carries_information() {
        let mut f = frame();
        f[10] = 62;
        f[11] = 0x4A; // continuous, length fixed at 0
        let s = Status::parse(f, Series::Ql).expect("valid frame");
        assert_eq!(s.media_description(), "continuous length tape, 62mm");

        f[11] = 0x4B; // die-cut
        f[17] = 29;
        let s = Status::parse(f, Series::Ql).expect("valid frame");
        assert_eq!(s.media_description(), "die-cut labels, 62x29mm");
    }

    #[test]
    fn status_type_covers_the_documented_values() {
        for (v, want) in [
            (0x00, StatusType::ReplyToRequest),
            (0x01, StatusType::PrintingCompleted),
            (0x02, StatusType::ErrorOccurred),
            (0x03, StatusType::ExitIfMode),
            (0x04, StatusType::TurnedOff),
            (0x05, StatusType::Notification),
            (0x06, StatusType::PhaseChange),
            (0x77, StatusType::Unknown(0x77)),
        ] {
            let mut f = frame();
            f[18] = v;
            assert_eq!(
                Status::parse(f, Series::Pt).expect("valid frame").kind,
                want
            );
        }
    }

    #[test]
    fn display_reads_as_ready_when_no_bit_is_set() {
        let mut f = frame();
        f[10] = 12;
        f[11] = 0x01;
        let s = Status::parse(f, Series::Pt).expect("valid frame");
        assert!(!s.has_error());
        assert_eq!(s.to_string(), "READY (laminated tape, 12mm)");
    }
}
