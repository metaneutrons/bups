# Contributing

## Commits

Conventional Commits, without exception. release-please derives the version and
the changelog from the commit types, so a commit outside the scheme produces a
wrong version or a missing changelog entry.

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`,
`ci`, `chore`, `revert`. Scope optional and lower case. Breaking change through
`!` after the type plus `BREAKING CHANGE:` in the body. Subject line at most
100 characters.

**The pull request title matters most.** Merges are squash merges and the pull
request title becomes the subject line on `main`, so that is what determines the
next version. CI checks it.

Branch names: `<type>/<short-description>`, for example `fix/usb-detach`.

## Before pushing

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --locked
cargo deny check
```

The git hooks run the fast half of this automatically. Install them with:

```bash
brew install lefthook gitleaks
lefthook install
```

If a hook is too slow for you, say so in an issue rather than reaching for
`--no-verify`. A hook nobody runs is worse than no hook.

## Toolchain

`rust-toolchain.toml` is the only source of the toolchain version. Do not pin a
different version in a workflow.

The MSRV is declared in `Cargo.toml` as `rust-version` and tested by its own CI
job. Raising it is a deliberate change, not a side effect.

## Hardware

Most of this project talks to USB printers, and CI has none. Anything touching
`src/usb/` needs a manual test against a real device; say in the pull request
which model you tested on.

Protocol parsing in `src/status.rs` is different: it is pure and must come with
unit tests. Byte offsets and flag values are checked against the Brother Raster
Command Reference, not against another implementation. The PT and QL series use
the same 32-byte frame with **different** meanings for several fields, so a
change to one parser is not automatically right for the other.

## Adding a printer

Device definitions live in one macro in `src/usb/device.rs`. Add the USB product
ID, the model name and the capabilities. If you cannot test the device yourself,
say so in the pull request; an untested entry is still useful, but it gets
labelled as untested.
