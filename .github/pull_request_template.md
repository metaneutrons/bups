<!--
The title must follow Conventional Commits. On a squash merge it becomes the
subject line on main and determines the next version. CI checks it.
-->

## What this changes

## Why

## How it was verified

<!--
Name what you actually ran, not what should pass. If it touches src/usb/, say
which printer model you tested against; CI has no hardware.
-->

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-features --locked`
- [ ] `cargo deny check`
- [ ] tested against a physical printer (model: )
