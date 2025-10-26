# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]


## [0.3.0](https://github.com/nick42d/cosmic-applet-arch/compare/arch-updates-rs/v0.2.2...arch-updates-rs/v0.3.0) - 2025-10-26

### Fixed
- Correction for PR #43 ([#44](https://github.com/nick42d/cosmic-applet-arch/pull/44))
- Filter devel packages from aur updates ([#43](https://github.com/nick42d/cosmic-applet-arch/pull/43))
- Upgrade to srcinfo v2.0.0 and handle architecture specific fields ([#38](https://github.com/nick42d/cosmic-applet-arch/pull/38))

### Other
- Update license ([#39](https://github.com/nick42d/cosmic-applet-arch/pull/39))




## [0.2.2](https://github.com/nick42d/cosmic-applet-arch/compare/arch-updates-rs/v0.2.1...arch-updates-rs/v0.2.2) - 2025-06-28

### Other
- Update deps ([#23](https://github.com/nick42d/cosmic-applet-arch/pull/23))




## [0.2.1](https://github.com/nick42d/cosmic-applet-arch/compare/arch-updates-rs/v0.2.0...arch-updates-rs/v0.2.1) - 2025-04-30

### Fixed
- Resolve checkupdates errors when run in parallel ([#17](https://github.com/nick42d/cosmic-applet-arch/pull/17))

## [0.2.0](https://github.com/nick42d/cosmic-applet-arch/compare/arch-updates-rs/v0.1.2...arch-updates-rs/v0.2.0) - 2025-03-05

### Added
- BREAKING CHANGE: Pacman updates now provide SourceRepo. Impact: Update struct split into AurUpdate and PacmanUpdate, check_pacman_updates now uses cache, and a new error variant is added. ([#6](https://github.com/nick42d/cosmic-applet-arch/pull/6))

### Fixed
- not finding a manually installed package ending with DEVEL_SUFFIXES shouldn't panic, closes #11 ([#12](https://github.com/nick42d/cosmic-applet-arch/pull/12))
- removed an unnecessary unwrap


