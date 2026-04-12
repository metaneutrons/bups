// bups - the print server for USB-based label printers
// Copyright (C) 2026 Fabian Schmieder
// SPDX-License-Identifier: GPL-3.0-or-later

//! PID file management for daemon mode.

use crate::error::{Error, Result};

/// RAII guard that writes a PID file on creation and removes it on drop.
pub struct PidFileGuard(String);

impl PidFileGuard {
    /// Create a PID file at `path`, failing if another instance is running.
    pub fn create(path: &str) -> Result<Self> {
        if let Ok(contents) = std::fs::read_to_string(path) {
            if let Ok(pid) = contents.trim().parse::<i32>() {
                if process_exists(pid) {
                    return Err(Error::PidFile(format!(
                        "another instance already running (PID {pid})"
                    )));
                }
            }
            // Stale PID file — remove it.
            let _ = std::fs::remove_file(path);
        }

        std::fs::write(path, std::process::id().to_string())
            .map_err(|e| Error::PidFile(format!("write failed: {e}")))?;

        Ok(Self(path.to_owned()))
    }
}

impl Drop for PidFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Check whether a process with the given PID is still running.
#[cfg(unix)]
fn process_exists(pid: i32) -> bool {
    // SAFETY: `kill(pid, 0)` sends no signal — it only checks existence.
    #[allow(unsafe_code)]
    unsafe {
        libc::kill(pid, 0) == 0
    }
}

#[cfg(not(unix))]
fn process_exists(_pid: i32) -> bool {
    false
}
