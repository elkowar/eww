# Eww - Widgets for everyone!

Eww (ElKowar's Wacky Widgets, pronounced with sufficient amounts of disgust)
is a widget system made in [Rust](https://www.rust-lang.org/),
which lets you create your own widgets similarly to how you can in AwesomeWM.
The key difference: It is independent of your window manager!

Configured in yuck and themed using CSS, it is easy to customize and provides all the flexibility you need!


## How to install Eww

### Prerequisites

* rustc
* cargo

Rather than with your system package manager,
I **strongly** recommend installing it using  [rustup](https://rustup.rs/).

Additionally, eww requires some dynamic libraries to be available on your system.
The exact names of the packages that provide these may differ depending on your distribution.
The following list of package names should work for arch linux:

<details>
<summary>Packages</summary>

- gtk3 (libgdk-3, libgtk-3)
- gtk-layer-shell (only on Wayland)
- pango (libpango)
- gdk-pixbuf2 (libgdk_pixbuf-2)
- libdbusmenu-gtk3
- cairo (libcairo, libcairo-gobject)
- glib2 (libgio, libglib-2, libgobject-2)
- gcc-libs (libgcc)
- glibc

</details>

(Note that you will most likely need the -devel variants of your distro's packages to be able to compile eww.)

### Building

Once you have the prerequisites ready, you're ready to install and build eww.

First clone the repo:
```bash
git clone https://github.com/elkowar/eww
```

```bash
cd eww
```
Then build:
```bash
cargo build --release --no-default-features --features x11
```
**NOTE:**
When you're on Wayland, build with:
```bash
cargo build --release --no-default-features --features=wayland
```

### Running eww
Once you've built it you can now run it by entering:
```bash
cd target/release
```
Then make the Eww binary executable:
```bash
chmod +x ./eww
```
Then to run it, enter:
```
./eww daemon
./eww open <window_name>
```
