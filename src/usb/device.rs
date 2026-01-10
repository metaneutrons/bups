// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! USB device definitions for Brother PT/QL printers.
//!
//! PIDs sourced from <http://www.linux-usb.org/usb.ids>

use bitflags::bitflags;

bitflags! {
    /// Printer capabilities for mDNS advertisement.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Capabilities: u8 {
        const COLOR = 0b0000_0001;
        const COPIES = 0b0000_0010;
        const DUPLEX = 0b0000_0100;
        const PAPER_CUSTOM = 0b0000_1000;
        const BINARY = 0b0001_0000;
        const TRANSPARENT = 0b0010_0000;
        const TBCP = 0b0100_0000;
    }
}

/// Supported printer devices.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Device {
    // PT Series (TZe tape)
    Pt18R,
    Pt1230Pc,
    Pt2420Pc,
    Pt2430Pc,
    Pt2730,
    Pt7600,
    PtD600,
    PtE550W,
    PtP700,
    PtP750W,
    // QL Series (labels)
    Ql500,
    Ql550,
    Ql560,
    Ql570,
    Ql600,
    Ql650Td,
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
    Ql1115NWB,
}

impl Device {
    /// Look up device from USB product ID (printer mode only, not mass storage).
    pub fn from_pid(pid: u16) -> Option<Self> {
        match pid {
            // PT Series
            0x201a => Some(Self::Pt18R),
            0x202c => Some(Self::Pt1230Pc),
            0x2007 => Some(Self::Pt2420Pc),
            0x202d => Some(Self::Pt2430Pc),
            0x2041 => Some(Self::Pt2730),
            0x202b => Some(Self::Pt7600),
            0x2074 => Some(Self::PtD600),
            0x2060 => Some(Self::PtE550W),
            0x2061 => Some(Self::PtP700),
            0x2065 => Some(Self::PtP750W),
            // QL Series
            0x2015 => Some(Self::Ql500),
            0x2016 => Some(Self::Ql550),
            0x2027 => Some(Self::Ql560),
            0x2028 => Some(Self::Ql570),
            0x20c0 => Some(Self::Ql600),
            0x201b => Some(Self::Ql650Td),
            0x2042 => Some(Self::Ql700),
            0x2043 => Some(Self::Ql710W),
            0x2044 => Some(Self::Ql720NW),
            0x209b => Some(Self::Ql800),
            0x209c => Some(Self::Ql810W),
            0x209d => Some(Self::Ql820NWB),
            0x2020 => Some(Self::Ql1050),
            0x202a => Some(Self::Ql1060N),
            0x20a7 => Some(Self::Ql1100),
            0x20a8 => Some(Self::Ql1110NWB),
            0x20ab => Some(Self::Ql1115NWB),
            _ => None,
        }
    }

    /// Get model name for display.
    pub fn model_name(&self) -> &'static str {
        match self {
            Self::Pt18R => "PT-18R",
            Self::Pt1230Pc => "PT-1230PC",
            Self::Pt2420Pc => "PT-2420PC",
            Self::Pt2430Pc => "PT-2430PC",
            Self::Pt2730 => "PT-2730",
            Self::Pt7600 => "PT-7600",
            Self::PtD600 => "PT-D600",
            Self::PtE550W => "PT-E550W",
            Self::PtP700 => "PT-P700",
            Self::PtP750W => "PT-P750W",
            Self::Ql500 => "QL-500",
            Self::Ql550 => "QL-550",
            Self::Ql560 => "QL-560",
            Self::Ql570 => "QL-570",
            Self::Ql600 => "QL-600",
            Self::Ql650Td => "QL-650TD",
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
            Self::Ql1115NWB => "QL-1115NWB",
        }
    }

    /// Get default capabilities for this device.
    pub fn capabilities(&self) -> Capabilities {
        Capabilities::BINARY | Capabilities::TRANSPARENT
    }
}
