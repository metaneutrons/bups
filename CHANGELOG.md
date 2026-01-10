# Changelog

All notable changes to this project will be documented in this file.

## [0.1.0] - 2026-01-10

### Added

- Initial commit - USB print server for label printers
- Add --snmp-port CLI option
- Add --version flag and version in startup log

### Documentation

- Update README with installation methods and CI badge
- Improve module-level documentation
- Add --snmp-port to README usage
- Add crates.io and AUR badges

### Fixed

- Use macos-15-intel runner for x86_64 builds

### Refactored

- Eliminate magic numbers, use bitflags for capabilities

### Build

- Add AUR, crates.io, and Homebrew publishing
- Update GitHub Actions to latest versions

