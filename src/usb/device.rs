// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! USB device definitions for Brother PT/QL printers.
//!
//! Maps USB product IDs to device types and provides model metadata.

/// Supported printer devices.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Device {
    // PT Series (TZe tape)
    Pt1230Pc,
    Pt2430Pc,
    PtD450,
    PtD600,
    PtE550W,
    PtE560BT,
    PtP700,
    PtP710Bt,
    PtP750W,
    PtP900W,
    PtP950NW,
    // QL Series (labels)
    Ql500,
    Ql550,
    Ql560,
    Ql570,
    Ql700,
    Ql710W,
    Ql720NW,
    Ql800,
    Ql810W,
    Ql820NWB,
    Ql1050,
    Ql1060N,
    Ql1100,
    Ql1110NWB,
}

impl Device {
    /// Look up device from USB product ID (printer mode).
    pub fn from_pid(pid: u16) -> Option<Self> {
        match pid {
            // PT Series
            0x202c => Some(Self::Pt1230Pc),
            0x202d => Some(Self::Pt2430Pc),
            0x2073 => Some(Self::PtD450),
            0x2074 => Some(Self::PtD600),
            0x2060 => Some(Self::PtE550W),
            0x2203 => Some(Self::PtE560BT),
            0x2061 => Some(Self::PtP700),
            0x20af => Some(Self::PtP710Bt),
            0x2062 => Some(Self::PtP750W),
            0x207c => Some(Self::PtP900W),
            0x207d => Some(Self::PtP950NW),
            // QL Series (including alternate PIDs)
            0x2018 => Some(Self::Ql500),
            0x2019 => Some(Self::Ql550),
            0x2015 | 0x2027 => Some(Self::Ql560),
            0x2016 | 0x2028 => Some(Self::Ql570),
            0x2017 | 0x2042 => Some(Self::Ql700),
            0x2043 => Some(Self::Ql710W),
            0x2044 => Some(Self::Ql720NW),
            0x209b => Some(Self::Ql800),
            0x209c => Some(Self::Ql810W),
            0x209d => Some(Self::Ql820NWB),
            0x2029 => Some(Self::Ql1050),
            0x202a => Some(Self::Ql1060N),
            0x20a7 => Some(Self::Ql1100),
            0x20a8 => Some(Self::Ql1110NWB),
            _ => None,
        }
    }

    /// Get model name for display.
    pub fn model_name(&self) -> &'static str {
        match self {
            Self::Pt1230Pc => "PT-1230PC",
            Self::Pt2430Pc => "PT-2430PC",
            Self::PtD450 => "PT-D450",
            Self::PtD600 => "PT-D600",
            Self::PtE550W => "PT-E550W",
            Self::PtE560BT => "PT-E560BT",
            Self::PtP700 => "PT-P700",
            Self::PtP710Bt => "PT-P710BT",
            Self::PtP750W => "PT-P750W",
            Self::PtP900W => "PT-P900W",
            Self::PtP950NW => "PT-P950NW",
            Self::Ql500 => "QL-500",
            Self::Ql550 => "QL-550",
            Self::Ql560 => "QL-560",
            Self::Ql570 => "QL-570",
            Self::Ql700 => "QL-700",
            Self::Ql710W => "QL-710W",
            Self::Ql720NW => "QL-720NW",
            Self::Ql800 => "QL-800",
            Self::Ql810W => "QL-810W",
            Self::Ql820NWB => "QL-820NWB",
            Self::Ql1050 => "QL-1050",
            Self::Ql1060N => "QL-1060N",
            Self::Ql1100 => "QL-1100",
            Self::Ql1110NWB => "QL-1110NWB",
        }
    }

    /// Printer capabilities for mDNS advertisement.
    pub fn capabilities(&self) -> Capabilities {
        Capabilities {
            color: false,
            copies: false,
            duplex: false,
            paper_custom: true,
            binary: true,
            transparent: false,
            tbcp: true,
        }
    }
}

/// Printer capabilities for mDNS.
pub struct Capabilities {
    pub color: bool,
    pub copies: bool,
    pub duplex: bool,
    pub paper_custom: bool,
    pub binary: bool,
    pub transparent: bool,
    pub tbcp: bool,
}
