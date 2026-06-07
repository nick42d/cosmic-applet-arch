# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]


## [1.0.0](https://github.com/nick42d/cosmic-applet-arch/releases/tag/cosmic-applet-arch/v1.0.0) - 2026-06-07

### Added
- Seperate AUR errors from pacman errors ([#46](https://github.com/nick42d/cosmic-applet-arch/pull/46))
- Underline package names if they have a URL for clarity ([#20](https://github.com/nick42d/cosmic-applet-arch/pull/20))
- Allow user to specify package urls for unofficial repos ([#36](https://github.com/nick42d/cosmic-applet-arch/pull/36))
- add refreshing icon when refreshing updates ([#35](https://github.com/nick42d/cosmic-applet-arch/pull/35))
- selectively hide update types from count ([#32](https://github.com/nick42d/cosmic-applet-arch/pull/32))
- Adjustable refresh times ([#31](https://github.com/nick42d/cosmic-applet-arch/pull/31))
- Add config ([#30](https://github.com/nick42d/cosmic-applet-arch/pull/30))
- Show latest news from Arch rss feed ([#9](https://github.com/nick42d/cosmic-applet-arch/pull/9))
- Ability to open AUR page as hyperlink - closes #5 ([#6](https://github.com/nick42d/cosmic-applet-arch/pull/6))
- Update iced to epoch 3, handle vertical layout

### Fixed
- *(i18n)* move pt-br translation to correct folder ([#53](https://github.com/nick42d/cosmic-applet-arch/pull/53))

- Fix auto localize ([#27](https://github.com/nick42d/cosmic-applet-arch/pull/27))
- Fix auto localize ([#27](https://github.com/nick42d/cosmic-applet-arch/pull/27))
- ensure local data storage directory is always created instead of erroring ([#25](https://github.com/nick42d/cosmic-applet-arch/pull/25))
- Resolve checkupdates errors when run in parallel / errors shown on applet when on multiple screens ([#17](https://github.com/nick42d/cosmic-applet-arch/pull/17))
- Links should spawn a detached process if browser isnt already open, to avoid blocking app ([#15](https://github.com/nick42d/cosmic-applet-arch/pull/15))
- Applet menu looks like loading state when actually error state ([#7](https://github.com/nick42d/cosmic-applet-arch/pull/7))

### Other
- Use a version num without `beta` ([#71](https://github.com/nick42d/cosmic-applet-arch/pull/71))
- Add an id for release-plz step and make version number AUR appropriate ([#70](https://github.com/nick42d/cosmic-applet-arch/pull/70))
- *(cosmic-applet-arch)* release v1.0.0-beta.17 ([#69](https://github.com/nick42d/cosmic-applet-arch/pull/69))

- Put release true for applet ([#68](https://github.com/nick42d/cosmic-applet-arch/pull/68))
- Bump vernum ([#67](https://github.com/nick42d/cosmic-applet-arch/pull/67))
- Update deps ([#63](https://github.com/nick42d/cosmic-applet-arch/pull/63))
- Rename pt-BR folder ([#54](https://github.com/nick42d/cosmic-applet-arch/pull/54))
- Update DE translation following #46 ([#49](https://github.com/nick42d/cosmic-applet-arch/pull/49))
- Update to latest libcosmic ([#45](https://github.com/nick42d/cosmic-applet-arch/pull/45))
- Brazilian Portuguese translation files added ([#52](https://github.com/nick42d/cosmic-applet-arch/pull/52))
- Swedish update ([#50](https://github.com/nick42d/cosmic-applet-arch/pull/50))
- Update license ([#39](https://github.com/nick42d/cosmic-applet-arch/pull/39))
- Update to latest libcosmic ([#23](https://github.com/nick42d/cosmic-applet-arch/pull/23))
- update version and lockfile
- Revert "chore: update version and lockfile"
- Revert "fix: Fix auto localize ([#27](https://github.com/nick42d/cosmic-applet-arch/pull/27))"
- update version and lockfile
- added german ([#26](https://github.com/nick42d/cosmic-applet-arch/pull/26))
- *(arch-updates-rs)* release v0.2.1 ([#18](https://github.com/nick42d/cosmic-applet-arch/pull/18))

- Revert "chore: Update to latest - in line with cosmic alpha 7 epoch ([#19](https://github.com/nick42d/cosmic-applet-arch/pull/19))" ([#22](https://github.com/nick42d/cosmic-applet-arch/pull/22))
- Update to latest - in line with cosmic alpha 7 epoch ([#19](https://github.com/nick42d/cosmic-applet-arch/pull/19))
- Bump vernum
- Update deps and lockfile
- Fix Swedish translation ([#14](https://github.com/nick42d/cosmic-applet-arch/pull/14))
- Bump vernum
- *(arch-updates-rs)* release v0.2.0 ([#8](https://github.com/nick42d/cosmic-applet-arch/pull/8))

- Update sv .ftl ([#13](https://github.com/nick42d/cosmic-applet-arch/pull/13))
- add Swedish translation ([#10](https://github.com/nick42d/cosmic-applet-arch/pull/10))
- Fixed .desktop entry showing this as a file browser
- Update lockfile
- Update vernums
- Update libcosmic
- Bump vernum again
- Add mock api implementation
- Update version number
- Show error icon if initial load failed
- Complete API documentation
- Improve timeout duration, resolve glitchy devel updates
- Implement timeout
- Merge ticker strategy
- offline checks should only need to take cache by reference, and not return cache.
- Remove unwraps from subscription
- Show error icon if required
- In progress- add error handling
- Tidy up of unused imports etcc
- Use locale specific datetime
- Fix not getting devel upgrades correctly, slightly widen (matches notifications applet)
- Complete library
- Implement force updater, and overflow rows
- Add loading icon
- Fix divider padding, localtime
- Resolve - applet cut off
- Resolve - button looks pressed
- Resolve - popup is displaying the wrong colour
- UI layout work
- Implement first set of i18n
- Playing with custom widgets
- Refactor view and subscription
- Continue improving layout
- Playing with layout
- Select acceptable design, add icon
- Experiment with custom icon
- Integrate new api modes with applet
- All api modes implemented now
- Add working button widget
- First working version



## [1.0.0-beta.17](https://github.com/nick42d/cosmic-applet-arch/releases/tag/cosmic-applet-arch/v1.0.0-beta.17) - 2026-06-07

### Fixed
- Update deps [resolves crash on latest cosmic] ([#63](https://github.com/nick42d/cosmic-applet-arch/pull/63))

## [1.0.0.beta.16](https://github.com/nick42d/cosmic-applet-arch/compare/arch-updates-rs/v0.3.1...cosmic-applet-arch-v1.0.0.beta.16) - 2025-11-20

### Added
- Fix pt-BR folder by renaming it by @Azure-Orit ([#54](https://github.com/nick42d/cosmic-applet-arch/pull/54))
