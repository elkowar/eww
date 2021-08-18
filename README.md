

# Eww

<img src="./.github/EwwLogo.svg" height="100" align="left"/>

Elkowars Wacky Widgets is a standalone widget system made in Rust that allows you to implement
your own, custom widgets in any window manager.

Documentation **and instructions on how to install** can be found [here](https://elkowar.github.io/eww).

## New configuration language!

YUCK IS ALIVE! After months of waiting, the new configuration language has now been released!
This also means that XML is no longer supported from this point onwards.
If you want to keep using the latest releases of eww, you'll need to migrate your config over to yuck.

The steps to migrate can be found in [the migration guide](YUCK_MIGRATION.md)

Additionally, a couple _amazing_ people have started to work on an
[automatic converter](https://github.com/undefinedDarkness/ewwxml) that can turn your old eww.xml into the new yuck format!


## Examples

(note that some of these still make use of the old configuration syntax)

* A basic bar, see [examples](./examples/eww-bar)
![Example 1](./examples/eww-bar/eww-bar.png)

* [Setup by Axarva](https://github.com/Axarva/dotfiles-2.0)
![Axarva-rice](https://raw.githubusercontent.com/Axarva/dotfiles-2.0/main/screenshots/center.png)

* [Setup by adi1090x](https://github.com/adi1090x/widgets)
![Nordic](https://raw.githubusercontent.com/adi1090x/widgets/main/previews/nordic.png)

## Contribewwting

If you want to contribute anything, like adding new widgets, features or subcommands (Including sample configs), you should definitely do so.

### Steps
1. Fork this repository
2. Install dependencies
3. Smash your head against the keyboard from frustration (coding is hard)
4. Open a pull request once you're finished
