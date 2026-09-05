# Changelog

All notable changes to this project will be documented in this file.

## [0.3.1](https://github.com/metaneutrons/bups/compare/v0.3.0...v0.3.1) (2026-09-05)


### Bug Fixes

* **ci:** six pipelines could fail or lie through SIGPIPE ([#38](https://github.com/metaneutrons/bups/issues/38)) ([b0e28a2](https://github.com/metaneutrons/bups/commit/b0e28a255049c4c95bd2fc41239db8fe77a15c02))

## [0.3.0](https://github.com/metaneutrons/bups/compare/v0.2.0...v0.3.0) (2026-09-05)


### ⚠ BREAKING CHANGES

* replace release-plz with a hardened release-please pipeline ([#24](https://github.com/metaneutrons/bups/issues/24))

### Features

* **cli:** --version reports commit, target, toolchain and build time ([#29](https://github.com/metaneutrons/bups/issues/29)) ([5a0f223](https://github.com/metaneutrons/bups/commit/5a0f223ca9f5bf6bdfb3c302759623e9685e8b11))
* **release:** publish through dedicated Apps instead of stored PATs ([#36](https://github.com/metaneutrons/bups/issues/36)) ([14ec338](https://github.com/metaneutrons/bups/commit/14ec338c4f38a53c918fe9e30ae0f1e1caadaf34))
* **release:** verify what the archive serves after the dispatch ([#34](https://github.com/metaneutrons/bups/issues/34)) ([e97d155](https://github.com/metaneutrons/bups/commit/e97d155596ba4eae4aa55e31f3a5c76d9cf050f8))


### Bug Fixes

* **apt:** follow the archive to shared-root-v1 ([#37](https://github.com/metaneutrons/bups/issues/37)) ([3502ce9](https://github.com/metaneutrons/bups/commit/3502ce9ea33ac23a11505689a344be2707089919))
* **cli:** --model matches every documented name of a device ([#27](https://github.com/metaneutrons/bups/issues/27)) ([d775589](https://github.com/metaneutrons/bups/commit/d775589fae2a43657983741a2f135f1fc3740b62))
* **deps:** resolve three RUSTSEC advisories and raise MSRV to 1.95 ([#8](https://github.com/metaneutrons/bups/issues/8)) ([9cc1dec](https://github.com/metaneutrons/bups/commit/9cc1dec2585bdc68b5c502012eba1cd0dc3384a4))
* **release:** anchor release-please at the last actually released commit ([#31](https://github.com/metaneutrons/bups/issues/31)) ([6dfd808](https://github.com/metaneutrons/bups/commit/6dfd8089a3fddcfd734a9e36cfb6252e64cc23d4))
* **release:** move last-release-sha to the manifest root, where it works ([#32](https://github.com/metaneutrons/bups/issues/32)) ([53eda5f](https://github.com/metaneutrons/bups/commit/53eda5fe576c6344207d67b0000e1cbba33bef70))
* **release:** skip an unconfigured channel instead of failing the release ([#33](https://github.com/metaneutrons/bups/issues/33)) ([40a3db4](https://github.com/metaneutrons/bups/commit/40a3db4cdfcad425b2e9a98ca437970fc1ac3da5))
* **snmp:** answer only for the OID that is actually served ([#15](https://github.com/metaneutrons/bups/issues/15)) ([6b3dbe1](https://github.com/metaneutrons/bups/commit/6b3dbe1702ed4d9948ba71a8c07efecc14537970))
* **status:** parse PT and QL status frames separately ([#23](https://github.com/metaneutrons/bups/issues/23)) ([4b2d09c](https://github.com/metaneutrons/bups/commit/4b2d09c2f0699e0f5a55c2b41ab69e53d085f9c0))
* **tcp:** hold the printer for a whole job, stop polling status per chunk ([#25](https://github.com/metaneutrons/bups/issues/25)) ([8bb26e7](https://github.com/metaneutrons/bups/commit/8bb26e799a30e90b27525e98e9db353468bc497a))
* **usb:** detach the usblp kernel driver, add the PT-P900 ([#14](https://github.com/metaneutrons/bups/issues/14)) ([9fd658f](https://github.com/metaneutrons/bups/commit/9fd658f50edb53095263e69fa29dfcd1aa521568))


### Continuous Integration

* replace release-plz with a hardened release-please pipeline ([#24](https://github.com/metaneutrons/bups/issues/24)) ([6dd51ef](https://github.com/metaneutrons/bups/commit/6dd51ef66ded4e9b238df560e369c754bdb2ea4a))

## [0.2.0] - 2026-04-12

### Added

- Add PT-2300/2310 support

### Fixed

- Correct USB PIDs from linux-usb.org

### Refactored

- Enterprise-grade code quality overhaul


## [0.1.1] - 2026-01-10

### Added

- Add armv7-unknown-linux-musleabihf release target
