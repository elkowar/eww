# Changelog

All notable changes to eww will be listed here, starting at changes since version 0.2.0.

## Unreleased

### BREAKING CHANGES
- [#1176](https://github.com/elkowar/eww/pull/1176) changed safe access (`?.`) behavior:
  Attempting to index in an empty JSON string (`'""'`) is now an error.

### Fixes
- Re-enable some scss features (By: w-lfchen)
- Fix and refactor nix flake (By: w-lfchen)
- Fix remove items from systray (By: vnva)
- Fix the gtk `stack` widget (By: ovalkonia)
- Fix values in the `EWW_NET` variable (By: mario-kr)
- Fix the gtk `expander` widget (By: ovalkonia)
- Fix wayland monitor names support (By: dragonnn)
- `get_locale` now follows POSIX standard for locale selection (By: mirhahn, w-lfchen)

### Features
- Add OnDemand support for focusable on wayland (By: GallowsDove)
- Add jq `raw-output` support (By: RomanHargrave)
- Update rust toolchain to 1.81.0 (By: w-lfchen)
- Add `:fill-svg` and `:preserve-aspect-ratio` properties to images (By: hypernova7, w-lfchen)
- Add `:truncate` property to labels, disabled by default (except in cases where truncation would be enabled in version `0.5.0` and before) (By: Rayzeq).
- Add support for `:hover` css selectors for tray items (By: zeapoz)
- Add scss support for the `:style` widget property (By: ovalkonia)
- Add `min` and `max` function calls to simplexpr (By: ovalkonia)
- Add `flip-x`, `flip-y`, `vertical` options to the graph widget to determine its direction
- Add `transform-origin-x`/`transform-origin-y` properties to transform widget (By: mario-kr)
- Add keyboard support for button presses (By: julianschuler)
- Support empty string for safe access operator (By: ModProg)
- Add `log` function calls to simplexpr (By: topongo)
- Add support for `:keypress` for eventbox (By: AlexandrePicavet)

## [0.6.0] (21.04.2024)

### Fixes
- The `shell-completions` subcommand is now run before anything is set up
- Fix nix flake
- Fix `jq` (By: w-lfchen)
- Labels now use gtk's truncation system (By: Rayzeq).

### Features
- Add `systray` widget (By: ralismark)
- Add `:checked` property to checkbox (By: levnikmyskin)

## [0.5.0] (17.02.2024)

### BREAKING CHANGES
- Remove `eww windows` command, replace with `eww active-windows` and `eww list-windows`

### Features
- Add `:icon` and `:icon-size` to the image widget (By: Adrian Perez de Castro)
- Add `get_env` function (By: RegenJacob)
- Add `:namespace` window option
- Default to building with x11 and wayland support simultaneously
- Add `truncate-left` property on `label` widgets (By: kawaki-san)
- Add `gravity` property on `label` widgets (By: Elekrisk)
- Add support for safe access (`?.`) in simplexpr (By: oldwomanjosiah)
- Allow floating-point numbers in percentages for window-geometry
- Add support for safe access with index (`?.[n]`) (By: ModProg)
- Made `and`, `or` and `?:` lazily evaluated in simplexpr (By: ModProg)
- Add Vanilla CSS support (By: Ezequiel Ramis)
- Add `jq` function, offering jq-style json processing
- Add support for the `EWW_BATTERY` magic variable in FreeBSD, OpenBSD, and NetBSD (By: dangerdyke)
- Add `justify` property to the label widget, allowing text justification (By: n3oney)
- Add `EWW_TIME` magic variable (By: Erenoit)
- Add trigonometric functions (`sin`, `cos`, `tan`, `cot`) and degree/radian conversions (`degtorad`, `radtodeg`) (By: end-4)
- Add `substring` function to simplexpr
- Add `--duration` flag to `eww open`
- Add support for referring to monitor with `<primary>`
- Add support for multiple matchers in `monitor` field
- Add `stack` widget (By: vladaviedov)
- Add `unindent` property to the label widget, allowing to disable removal of leading spaces (By: nrv)
- Switch to stable rust toolchain (1.76)
- Add `tooltip` widget, which allows setting a custom tooltip (not only text), to a widget (By: Rayzeq)
- Add `eww shell-completions` command, generating completion scripts for different shells

### Fixes
- Fix wrong values in `EWW_NET`
- Fix logfiles growing indefinitely

## [0.4.0] (04.09.2022)

### BREAKING CHANGES
- Change `calendar`-widget to index months starting at 1 rather than indexed from 0

### Features
- Add support for output names in X11 to select `:monitor`.
- Add support for `:active`-pseudoselector on eventbox (By: viandoxdev)
- Add support for `:password` on input (By: viandoxdev)

### Notable fixes and other changes
- Scale now only runs the onchange command on changes caused by user-interaction
- Improve CSS error reporting
- Fix deflisten scripts not always getting cleaned up properly
- Add `:round-digits` to scale widget (By: gavynriebau)
- Fix cirular-progress not properly displaying 100% values when clockwise is false
- Fix temperatures inside `EWW_TEMPS` not being accessible if at least one value is `NaN`


## 0.3.0 (26.05.2022)

### BREAKING CHANGES
- Change the onclick command API to support multiple placeholders.
  This changes. the behaviour of the calendar widget's onclick as well as the onhover and onhoverlost
  events. Instead of providing the entire date (or, respecively, the x and y mouse coordinates) in
  a single value (`day.month.year`, `x y`), the values are now provided as separate placeholders.
  The day can now be accessed with `{0}`, the month with `{1}`, and the year with `{2}`, and
  similarly x and y are accessed with `{0}` and `{1}`.

### Features
- Add `eww inspector` command
- Add `--no-daemonize` flag
- Add support for displaying marks on `scale`-widget (By: druskus20)
- Add `children`-widget that allows custom widgets to make use of children
- Add support for `:hover` css selectors for eventbox (By: druskus20)
- Add `eww get` subcommand (By: druskus20)
- Add circular progress widget (By: druskus20)
- Add `:xalign` and `:yalign` to labels (By: alecsferra)
- Add `graph` widget (By: druskus20)
- Add `>=` and `<=` operators to simplexpr (By: viandoxdev)
- Add `desktop` window type (By: Alvaro Lopez)
- Add `scroll` widget (By: viandoxdev)
- Add `notification` window type
- Add drag and drop functionality to eventbox
- Add `search`, `captures`, `stringlength`, `arraylength` and `objectlength` functions for expressions (By: MartinJM, ElKowar)
- Add `matches` function
- Add `transform` widget (By: druskus20)
- Add `:onaccept` to input field, add `:onclick` to eventbox
- Add `EWW_CMD`, `EWW_CONFIG_DIR`, `EWW_EXECUTABLE` magic variables
- Add `overlay` widget (By: viandoxdev)
- Add arguments option to `defwindow` (By: WilfSilver)

### Notable Internal changes
- Rework state management completely, now making local state and dynamic widget hierarchy changes possible.

### Notable fixes and other changes
- Fix `onscroll` gtk-bug where the first event is emitted incorrectly (By: druskus20)
- Allow windows to get moved when windowtype is `normal`
- Added more examples
- List system-level dependencies in documentation
- Document structure of magic variables (By: legendofmiracles)
- Updated dependencies
