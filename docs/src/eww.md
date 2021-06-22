# Eww - Widgets for everyone!

Eww (ElKowar's Wacky Widgets, pronounced with sufficient amounts of disgust)
is a widget system made in [rust](https://www.rust-lang.org/),
which let's you create your own widgets similarly to how you can in AwesomeWM.
The key difference: It is independent of your window manager!

Configured in XML and themed using CSS, it is easy to customize and provides all the flexibility you need!


## How to install Eww

### Prerequisites

* rustc
* cargo (nightly toolchain)

Rather than with your system package manager,
I recommend installing it using  [rustup](https://rustup.rs/),
as this makes it easy to use the nightly toolchain necessary to build eww.

### Building

Once you have the Prerequisites ready, you're ready to install and build eww.

First clone the repo:
```bash
git clone https://github.com/elkowar/eww
```

```bash
cd eww
```
and then to build:
```bash
cargo build --release
```
**NOTE:**
When you're on wayland, build with:
```bash
cargo build --release --no-default-features --features=wayland
```

### Running eww
Once you've built it you can now run it by entering:
```bash
cd target/release
```
and then make the Eww binary executable
```bash
chmod +x ./eww
```
and then to run it do
```
./eww daemon
./eww open <window_name>
```
`<window_name>` is the name of the window, see [The windows block](configuration.md#windows-block).
