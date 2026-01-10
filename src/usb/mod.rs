// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! USB module.

pub mod device;
pub mod printer;

pub use device::Device;
pub use printer::{Printer, list_printers};
