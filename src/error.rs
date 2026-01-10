// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! Error types for bups.

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("USB error: {0}")]
    Usb(#[from] nusb::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("No printer found")]
    NoPrinter,

    #[error("USB transfer error: {0}")]
    Transfer(String),
}

pub type Result<T> = std::result::Result<T, Error>;
