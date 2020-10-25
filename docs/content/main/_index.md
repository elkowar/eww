+++
title = "Eww - Widgets for everyone!"
slug = "eww"
sort_by = "weight"
+++


Eww (ElKowar's Wacky Widgets, pronounced with sufficient amounts of disgust) Is a widgeting system made in [rust](https://www.rust-lang.org/), which let's you create your own widgets simmilarly to how you can in AwesomeWM. The key difference: It is independent of your window manager!

Configured in XML and themed using CSS, it is easy to customize and provides all the flexibility you need!


## How to install Eww

### Prerequisites

* rustc
* cargo (nightly toolchain)

Rather than with your system package manager, I recommend installing it using  [rustup](https://rustup.rs/), as this makes it easy to use the nightly toolchain, which is necessary to build eww.

### Building

Once you have the Prerequisites ready, you're ready to install and build eww.

First clone the repo:
```bash
git clone https://github.com/elkowar/eww
```
then enter it.
```bash
cd eww
```
and then to build:
```bash
cargo build --release
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
./eww open <window_name>
```
`<window_name>` is the name of the window, see [The windows block](@/main/configuration.md#windows-block).
