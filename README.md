

# Eww

<img src="./.github/EwwLogo.svg" height="100" align="left"/>

Elkowars Wacky Widgets is a standalone widget system made in Rust that allows you to implement
your own, custom widgets in any window manager.

Documentation **and instructions on how to install** can be found [here](https://elkowar.github.io/eww).

## New configuration language!

YUCK IS ALIVE! After months of waiting, the new configuration language has now been released!
This also means that XML is no longer supported from this point onwards.
If you want to keep using the latest releases of eww, you'll need to migrate your config over to yuck.

The steps to migrate can be found in [the migration guide](YUCK_MIGRATION.md).

Additionally, a couple _amazing_ people have started to work on an
[automatic converter](https://github.com/undefinedDarkness/ewwxml) that can turn your old eww.xml into the new yuck format!

## Examples

(Note that some of these still make use of the old configuration syntax.)

* A basic bar, see [examples](./examples/eww-bar)
![Example 1](./examples/eww-bar/eww-bar.png)

* [Some setups by Druskus20](https://github.com/druskus20/eugh)
![Druskus20-bar](https://raw.githubusercontent.com/druskus20/eugh/master/polybar-replacement/.github/preview.png)

* [My own vertical bar](https://github.com/elkowar/dots-of-war/tree/master/eww-bar/.config/eww-bar)

<img src="https://raw.githubusercontent.com/elkowar/dots-of-war/master/eww-bar/.config/eww-bar/showcase.png" height="400" width="auto"/>

* [Setup by Axarva](https://github.com/Axarva/dotfiles-2.0)
![Axarva-rice](https://raw.githubusercontent.com/Axarva/dotfiles-2.0/main/screenshots/center.png)

* [Setup by adi1090x](https://github.com/adi1090x/widgets)
![Nordic](https://raw.githubusercontent.com/adi1090x/widgets/main/previews/nordic.png)

* [i3 Bar replacement by owenrumney](https://github.com/owenrumney/eww-bar)
![Top bar](https://raw.githubusercontent.com/owenrumney/eww-bar/master/.github/topbar.gif)
![Bottom bar](https://raw.githubusercontent.com/owenrumney/eww-bar/master/.github/bottombar.gif)

* [Setups by iSparsh](https://github.com/iSparsh/gross)
![iSparsh-gross](https://user-images.githubusercontent.com/57213270/140309158-e65cbc1d-f3a8-4aec-848c-eef800de3364.png)


## Contribewwting

If you want to contribute anything, like adding new widgets, features, or subcommands (Including sample configs), you should definitely do so.

### Steps
1. Fork this repository
2. Install dependencies
3. Smash your head against the keyboard from frustration (coding is hard)
4. Write down your changes in CHANGELOG.md
5. Open a pull request once you're finished
