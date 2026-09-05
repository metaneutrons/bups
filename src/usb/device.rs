// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! USB device definitions for Brother PT/QL printers.
//!
//! PIDs sourced from <http://www.linux-usb.org/usb.ids>.
//! All device data is defined once via the [`define_devices!`] macro (SSOT).

use bitflags::bitflags;

/// Printer family.
///
/// The two families share the 32-byte status frame but assign several fields
/// different meanings, so status parsing has to know which one it is looking
/// at. See `crate::status`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Series {
    /// `P-touch`, `TZe` tape.
    Pt,
    /// `QL`, continuous tape and die-cut labels.
    Ql,
}

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
/// Format: `VariantName, pid, "Model Name", series, capabilities;`
macro_rules! define_devices {
    ($($variant:ident, $pid:expr, $name:expr, $series:expr, $caps:expr);+ $(;)?) => {
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

            /// Printer family, which decides how the status frame is read.
            #[must_use]
            pub const fn series(self) -> Series {
                match self {
                    $(Self::$variant => $series,)+
                }
            }

            /// The product ID this variant was declared with.
            ///
            /// Only used by the tests, to check the table against itself.
            #[cfg(test)]
            const fn product_id(self) -> u16 {
                match self {
                    $(Self::$variant => $pid,)+
                }
            }
        }
    };
}

define_devices! {
    // PT Series (TZe tape)
    Pt18R,     0x201a, "PT-18R", Series::Pt,       DEFAULT_CAPS;
    Pt1230Pc,  0x202c, "PT-1230PC", Series::Pt,    DEFAULT_CAPS;
    Pt2300,    0x2004, "PT-2300/2310", Series::Pt,  DEFAULT_CAPS;
    Pt2420Pc,  0x2007, "PT-2420PC", Series::Pt,     DEFAULT_CAPS;
    Pt2430Pc,  0x202d, "PT-2430PC", Series::Pt,     DEFAULT_CAPS;
    Pt2730,    0x2041, "PT-2730", Series::Pt,       DEFAULT_CAPS;
    Pt7600,    0x202b, "PT-7600", Series::Pt,       DEFAULT_CAPS;
    PtD600,    0x2074, "PT-D600", Series::Pt,       DEFAULT_CAPS;
    PtE550W,   0x2060, "PT-E550W", Series::Pt,     DEFAULT_CAPS;
    PtP700,    0x2061, "PT-P700", Series::Pt,       DEFAULT_CAPS;
    PtP750W,   0x2065, "PT-P750W", Series::Pt,     DEFAULT_CAPS;

    // QL Series (labels)
    Ql500,     0x2015, "QL-500", Series::Ql,       DEFAULT_CAPS;
    Ql550,     0x2016, "QL-550", Series::Ql,       DEFAULT_CAPS;
    Ql560,     0x2027, "QL-560", Series::Ql,       DEFAULT_CAPS;
    Ql570,     0x2028, "QL-570", Series::Ql,       DEFAULT_CAPS;
    Ql600,     0x20c0, "QL-600", Series::Ql,       DEFAULT_CAPS;
    Ql650Td,   0x201b, "QL-650TD", Series::Ql,     DEFAULT_CAPS;
    Ql700,     0x2042, "QL-700", Series::Ql,       DEFAULT_CAPS;
    Ql710W,    0x2043, "QL-710W", Series::Ql,      DEFAULT_CAPS;
    Ql720NW,   0x2044, "QL-720NW", Series::Ql,     DEFAULT_CAPS;
    Ql800,     0x209b, "QL-800", Series::Ql,       DEFAULT_CAPS.union(Capabilities::COLOR);
    Ql810W,    0x209c, "QL-810W", Series::Ql,      DEFAULT_CAPS.union(Capabilities::COLOR);
    Ql820NWB,  0x209d, "QL-820NWB", Series::Ql,    DEFAULT_CAPS.union(Capabilities::COLOR);
    Ql1050,    0x2020, "QL-1050", Series::Ql,      DEFAULT_CAPS;
    Ql1060N,   0x202a, "QL-1060N", Series::Ql,     DEFAULT_CAPS;
    Ql1100,    0x20a7, "QL-1100", Series::Ql,      DEFAULT_CAPS;
    Ql1110NWB, 0x20a8, "QL-1110NWB", Series::Ql,   DEFAULT_CAPS;
    Ql1115NWB, 0x20ab, "QL-1115NWB", Series::Ql,   DEFAULT_CAPS;
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "panicking is how a test reports failure"
)]
mod tests {
    use super::*;

    /// Every device the table knows, for the exhaustiveness checks below.
    const ALL: &[Device] = &[
        Device::Pt18R,
        Device::Pt1230Pc,
        Device::Pt2300,
        Device::Pt2420Pc,
        Device::Pt2430Pc,
        Device::Pt2730,
        Device::Pt7600,
        Device::PtD600,
        Device::PtE550W,
        Device::PtP700,
        Device::PtP750W,
        Device::Ql500,
        Device::Ql550,
        Device::Ql560,
        Device::Ql570,
        Device::Ql600,
        Device::Ql650Td,
        Device::Ql700,
        Device::Ql710W,
        Device::Ql720NW,
        Device::Ql800,
        Device::Ql810W,
        Device::Ql820NWB,
        Device::Ql1050,
        Device::Ql1060N,
        Device::Ql1100,
        Device::Ql1110NWB,
        Device::Ql1115NWB,
    ];

    #[test]
    fn an_unknown_product_id_is_not_a_device() {
        assert_eq!(Device::from_pid(0x0000), None);
        assert_eq!(Device::from_pid(0xffff), None);
        // PT-P900, reported in issue #7 but not yet in the table.
        assert_eq!(Device::from_pid(0x208e), None);
    }

    #[test]
    fn known_product_ids_round_trip() {
        assert_eq!(Device::from_pid(0x2060), Some(Device::PtE550W));
        assert_eq!(Device::from_pid(0x2042), Some(Device::Ql700));
        assert_eq!(Device::from_pid(0x209d), Some(Device::Ql820NWB));
    }

    /// A duplicated product ID would make the earlier arm shadow the later one
    /// silently, and the wrong model name would be advertised over mDNS.
    #[test]
    fn no_product_id_is_claimed_by_two_devices() {
        for (i, a) in ALL.iter().enumerate() {
            for b in &ALL[i + 1..] {
                assert_ne!(
                    Device::product_id(*a),
                    Device::product_id(*b),
                    "{a:?} and {b:?} share a product ID"
                );
            }
        }
    }

    #[test]
    fn every_device_resolves_back_from_its_own_product_id() {
        for d in ALL {
            assert_eq!(
                Device::from_pid(Device::product_id(*d)),
                Some(*d),
                "{d:?} does not round-trip"
            );
        }
    }

    #[test]
    fn model_names_are_present_and_unique() {
        let mut seen: Vec<&str> = Vec::new();
        for d in ALL {
            let name = d.model_name();
            assert!(!name.is_empty(), "{d:?} has no model name");
            assert!(!seen.contains(&name), "duplicate model name {name}");
            seen.push(name);
        }
    }

    /// The series decides how the status frame is read, so a wrong entry
    /// silently mislabels every error the printer reports.
    #[test]
    fn the_series_follows_the_model_name() {
        for d in ALL {
            let expected = if d.model_name().starts_with("QL-") {
                Series::Ql
            } else {
                Series::Pt
            };
            assert_eq!(
                d.series(),
                expected,
                "{} has the wrong series",
                d.model_name()
            );
        }
    }

    #[test]
    fn every_device_advertises_at_least_the_default_capabilities() {
        for d in ALL {
            assert!(
                d.capabilities().contains(DEFAULT_CAPS),
                "{} lacks the default capabilities",
                d.model_name()
            );
        }
    }

    #[test]
    fn colour_is_claimed_only_by_the_models_that_have_it() {
        for d in ALL {
            let colour = d.capabilities().contains(Capabilities::COLOR);
            let expected = matches!(d, Device::Ql800 | Device::Ql810W | Device::Ql820NWB);
            assert_eq!(colour, expected, "{} colour capability", d.model_name());
        }
    }
}
