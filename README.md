# bups - Brother USB Print Server

[![CI](https://github.com/metaneutrons/bups/actions/workflows/ci.yml/badge.svg)](https://github.com/metaneutrons/bups/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/bups.svg)](https://crates.io/crates/bups)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Rust](https://img.shields.io/badge/Rust-1.74+-orange.svg)](https://www.rust-lang.org/)
[![AUR](https://img.shields.io/aur/version/bups)](https://aur.archlinux.org/packages/bups)

A network print server for Brother PT (P-Touch) and QL label printers. Exposes USB-connected printers over the network via TCP port 9100, with mDNS discovery and SNMP status reporting.

## Features

- **TCP Print Server** (port 9100) - Raw print data forwarding with bidirectional status
- **mDNS Advertisement** - Automatic printer discovery via Bonjour/Avahi (re-advertises on reconnect)
- **SNMP Responder** (port 161) - Brother-compatible status queries
- **Auto-Reconnect** - Configurable health monitoring and reconnection
- **Daemon Mode** - PID file, syslog support, graceful shutdown (SIGTERM/SIGINT)
- **Pure Rust USB** - Uses [nusb](https://crates.io/crates/nusb) (no libusb dependency)
- **Static Binary** - Builds for musl targets (Alpine, OpenWrt, embedded Linux)

## Supported Printers

### PT Series (TZe Tape)
| Model | PID | Tested |
|-------|-----|--------|
| PT-1230PC | 0x202c | |
| PT-2430PC | 0x202d | |
| PT-D450 | 0x2073 | |
| PT-D600 | 0x2074 | |
| PT-E550W | 0x2060 | ✅ |
| PT-E560BT | 0x2203 | |
| PT-P700 | 0x2061 | |
| PT-P710BT | 0x20af | |
| PT-P750W | 0x2062 | |
| PT-P900W | 0x207c | |
| PT-P950NW | 0x207d | |

### QL Series (Labels)
| Model | PID | Tested |
|-------|-----|--------|
| QL-500 | 0x2018 | |
| QL-550 | 0x2019 | |
| QL-560 | 0x2015/0x2027 | ✅ |
| QL-570 | 0x2016/0x2028 | |
| QL-700 | 0x2017/0x2042 | |
| QL-710W | 0x2043 | |
| QL-720NW | 0x2044 | |
| QL-800 | 0x209b | |
| QL-810W | 0x209c | |
| QL-820NWB | 0x209d | |
| QL-1050 | 0x2029 | |
| QL-1060N | 0x202a | |
| QL-1100 | 0x20a7 | |
| QL-1110NWB | 0x20a8 | |

## Installation

### Homebrew (macOS)

```bash
brew tap metaneutrons/tap
brew install bups
```

### Cargo

```bash
cargo install bups
```

### AUR (Arch Linux)

```bash
yay -S bups
```

### From Source

```bash
cargo build --release
./target/release/bups --help
```

For a fully static musl binary (portable to Alpine, OpenWrt, etc.):

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

## Usage

```
bups - the print server for USB-based label printers

Usage: bups [OPTIONS]

Options:
  -p, --port <PORT>                  TCP port for print data [default: 9100]
      --snmp-port <SNMP_PORT>        SNMP port for status queries [default: 161]
  -d, --debug                        Enable debug logging
  -b, --bind <BIND>                  Bind address [default: [::]]
      --model <MODEL>                Filter by model name (e.g. PT-E550W)
      --serial <SERIAL>              Filter by serial number
      --hostname <HOSTNAME>          Hostname for mDNS advertisement
  -l, --list                         List connected printers and exit
      --reconnect-interval <SECS>    Reconnect check interval [default: 30]
      --max-reconnects <N>           Max reconnect attempts (0 = infinite) [default: 0]
      --pid-file <PATH>              Write PID to file (for daemon mode)
      --syslog                       Log to syslog instead of stderr
  -h, --help                         Print help
```

### Examples

List connected printers:
```bash
bups -l
```

Start server with debug logging:
```bash
bups -d
```

Start server for specific printer:
```bash
bups --model PT-E550W --serial 000E9Z931020
```

Custom mDNS hostname:
```bash
bups --hostname MyLabelPrinter
```

### TCP Commands

Connect via netcat to send commands:

```bash
echo "STATUS" | nc localhost 9100
echo "HELP" | nc localhost 9100
```

## Running as a Daemon

bups runs in foreground by default (modern systemd style). No `--daemon` flag needed.

### With systemd (recommended)

Create `/etc/systemd/system/bups.service`:

```ini
[Unit]
Description=Brother USB Print Server
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/bups --syslog
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Then:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now bups
```

### Testing / Development

```bash
bups -d  # Debug logging to stderr
```

### With traditional init (SysV, OpenRC)

```bash
bups --pid-file /var/run/bups.pid --syslog &
```

For proper daemonization with traditional init, use a process supervisor or wrapper like `start-stop-daemon`:

```bash
start-stop-daemon --start --background --make-pidfile \
  --pidfile /var/run/bups.pid --exec /usr/local/bin/bups -- --syslog
```

The daemon will:
- Start even if no printer is connected (waits for USB hotplug events)
- Automatically reconnect when printer is unplugged/replugged (instant via USB hotplug)
- Re-advertise via mDNS when printer changes
- Clean up PID file on shutdown
- Handle SIGTERM/SIGINT (Ctrl+C) gracefully

## Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Print Client   │────▶│      bups       │────▶│  Brother USB    │
│  (macOS/Linux)  │◀────│   TCP:9100      │◀────│    Printer      │
└─────────────────┘     │   SNMP:161      │     └─────────────────┘
                        │   mDNS          │
        ┌───────────────┴─────────────────┴───────────────┐
        │                                                 │
   ┌────▼────┐    ┌──────────┐    ┌──────────┐    ┌──────▼──────┐
   │   TCP   │    │   SNMP   │    │   mDNS   │    │   Health    │
   │ Server  │    │Responder │    │ Advertise│    │   Check     │
   └─────────┘    └──────────┘    └──────────┘    └─────────────┘
```

## Dependencies

- [nusb](https://crates.io/crates/nusb) - Pure Rust USB library
- [tokio](https://crates.io/crates/tokio) - Async runtime
- [mdns-sd](https://crates.io/crates/mdns-sd) - mDNS/DNS-SD
- [rasn-snmp](https://crates.io/crates/rasn-snmp) - SNMP protocol
- [clap](https://crates.io/crates/clap) - CLI argument parsing
- [tracing](https://crates.io/crates/tracing) - Structured logging
- [syslog-tracing](https://crates.io/crates/syslog-tracing) - Syslog support (Unix)

## Credits

This project was heavily inspired by and based on the excellent work of **Ryan Kurte** on [rust-ptouch](https://github.com/ryankurte/rust-ptouch). The USB communication protocol, device definitions, and status parsing are derived from that project.

## License

GPL-3.0-or-later

Copyright (C) 2026 Fabian Schmieder

## See Also

- [rust-ptouch](https://github.com/ryankurte/rust-ptouch) - Brother P-Touch Raster Driver
- [Brother Raster Command Reference](https://download.brother.com/welcome/docp100064/cv_pte550wp750wp710bt_eng_raster_101.pdf)
