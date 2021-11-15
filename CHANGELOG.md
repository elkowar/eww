# Changelog

All notable changes to eww will be listed here, starting at changes since version 0.1.2.


## [Unreleased]

### Features
- Add `eww inspector` command
- Add `--no-daemonize` flag
- Add support for displaying marks on `scale`-widget (By: druskus20)
- Add `children`-widget that allows custom widgets to make use of children
- Add support for `:hover` css selectors for eventbox (By: druskus20)
- Add `eww get` subcommand (By: druskus20)
- Add circular progress widget (By: druskus20)

### Notable Internal changes
- Rework state management completely, now making local state and dynamic widget hierarchy changes possible.

### Notable fixes and other changes
- Fix `onscroll` gtk-bug where the first event is emitted incorrectly (By: druskus20)
- Allow windows to get moved when windowtype is `normal`
- Added more examples
- List system-level dependencies in documentation
- Document structure of magic variables (By: legendofmiracles)
