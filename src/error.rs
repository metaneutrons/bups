// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! Centralized error types for bups.

/// All errors that can occur in bups.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("USB error: {0}")]
    Usb(#[from] nusb::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("no printer found")]
    NoPrinter,

    #[error("USB transfer error: {0}")]
    Transfer(String),

    #[error("mutex poisoned")]
    MutexPoisoned,

    #[error("PID file error: {0}")]
    PidFile(String),
}

pub type Result<T> = std::result::Result<T, Error>;
