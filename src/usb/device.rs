// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! USB device definitions for Brother PT/QL printers.
//!
//! PIDs sourced from <http://www.linux-usb.org/usb.ids>.
//! All device data is defined once via the [`define_devices!`] macro (SSOT).

use bitflags::bitflags;

bitflags! {
    /// Printer capabilities advertised via mDNS TXT records.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Capabilities: u8 {
        const COLOR        = 0b0000_0001;
        const COPIES       = 0b0000_0010;
        const DUPLEX       = 0b0000_0100;
        const PAPER_CUSTOM = 0b0000_1000;
        const BINARY       = 0b0001_0000;
        const TRANSPARENT  = 0b0010_0000;
        const TBCP         = 0b0100_0000;
    }
}

/// Default capabilities shared by all current Brother label printers.
const DEFAULT_CAPS: Capabilities = Capabilities::BINARY.union(Capabilities::TRANSPARENT);

/// Single source of truth for all supported devices.
///
/// Format: `VariantName, pid, "Model Name", capabilities;`
macro_rules! define_devices {
    ($($variant:ident, $pid:expr, $name:expr, $caps:expr);+ $(;)?) => {
        /// Supported Brother printer devices.
        #[derive(Copy, Clone, Debug, PartialEq, Eq)]
        pub enum Device {
            $($variant),+
        }

        impl Device {
            /// Look up device from USB product ID.
            #[must_use]
            pub const fn from_pid(pid: u16) -> Option<Self> {
                match pid {
                    $($pid => Some(Self::$variant),)+
                    _ => None,
                }
            }

            /// Human-readable model name.
            #[must_use]
            pub const fn model_name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)+
                }
            }

            /// Capabilities for mDNS advertisement.
            #[must_use]
            pub const fn capabilities(self) -> Capabilities {
                match self {
                    $(Self::$variant => $caps,)+
                }
            }
        }
    };
}

define_devices! {
    // PT Series (TZe tape)
    Pt18R,     0x201a, "PT-18R",       DEFAULT_CAPS;
    Pt1230Pc,  0x202c, "PT-1230PC",    DEFAULT_CAPS;
    Pt2300,    0x2004, "PT-2300/2310",  DEFAULT_CAPS;
    Pt2420Pc,  0x2007, "PT-2420PC",     DEFAULT_CAPS;
    Pt2430Pc,  0x202d, "PT-2430PC",     DEFAULT_CAPS;
    Pt2730,    0x2041, "PT-2730",       DEFAULT_CAPS;
    Pt7600,    0x202b, "PT-7600",       DEFAULT_CAPS;
    PtD600,    0x2074, "PT-D600",       DEFAULT_CAPS;
    PtE550W,   0x2060, "PT-E550W",     DEFAULT_CAPS;
    PtP700,    0x2061, "PT-P700",       DEFAULT_CAPS;
    PtP750W,   0x2065, "PT-P750W",     DEFAULT_CAPS;

    // QL Series (labels)
    Ql500,     0x2015, "QL-500",       DEFAULT_CAPS;
    Ql550,     0x2016, "QL-550",       DEFAULT_CAPS;
    Ql560,     0x2027, "QL-560",       DEFAULT_CAPS;
    Ql570,     0x2028, "QL-570",       DEFAULT_CAPS;
    Ql600,     0x20c0, "QL-600",       DEFAULT_CAPS;
    Ql650Td,   0x201b, "QL-650TD",     DEFAULT_CAPS;
    Ql700,     0x2042, "QL-700",       DEFAULT_CAPS;
    Ql710W,    0x2043, "QL-710W",      DEFAULT_CAPS;
    Ql720NW,   0x2044, "QL-720NW",     DEFAULT_CAPS;
    Ql800,     0x209b, "QL-800",       DEFAULT_CAPS.union(Capabilities::COLOR);
    Ql810W,    0x209c, "QL-810W",      DEFAULT_CAPS.union(Capabilities::COLOR);
    Ql820NWB,  0x209d, "QL-820NWB",    DEFAULT_CAPS.union(Capabilities::COLOR);
    Ql1050,    0x2020, "QL-1050",      DEFAULT_CAPS;
    Ql1060N,   0x202a, "QL-1060N",     DEFAULT_CAPS;
    Ql1100,    0x20a7, "QL-1100",      DEFAULT_CAPS;
    Ql1110NWB, 0x20a8, "QL-1110NWB",   DEFAULT_CAPS;
    Ql1115NWB, 0x20ab, "QL-1115NWB",   DEFAULT_CAPS;
}
