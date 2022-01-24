

# Eww

<img src="./.github/EwwLogo.svg" height="100" align="left"/>

Elkowars Wacky Widgets is a standalone widget system made in Rust that allows you to implement
your own, custom widgets in any window manager.

Documentation **and instructions on how to install** can be found [here](https://elkowar.github.io/eww).

## Current maintenance status

To those of you looking at the current slow progress, commit frequency and general activity:
Don't worry, eww is not dead! I'm currently in the process of focussing on my bachelors thesis,
thus work on eww has currently slowed down significantly.
I hope to find more time to continue working on new features in the not-so-distant future.

Thanks for sticking around!

## Installing

If you would like to install eww the best way to do so would be to compile it manully. 

### Dependencies
In order to compile eww you must have the following packages
- rustc
- cargo (nightly toolchain)
- gtk3 (libgdk-3, libgtk-3)
- gtk-layer-shell (only on Wayland)
- pango (libpango)
- gdk-pixbuf2 (libgdk_pixbuf-2)
- cairo (libcairo, libcairo-gobject)
- glib2 (libgio, libglib-2, libgobject-2)
- gcc-libs (libgcc)
- glibc
### Compiling eww
```bash
cd eww
```
```bash
cargo build --release
```
**Note:** if your on wayland then do
```bash
cargo build --release --no-default-features --features=wayland
```
### Installing eww
```bash
cd target/release
```
```bash
chmod +x ./eww
```
```bash
sudo install --mode +x ./eww /usr/local/bin/
```

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
