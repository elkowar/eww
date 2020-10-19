# Table of Contents

-  [About](#org4ab08b6)
-  [Configuration](#org581ca61)
   -  [Example Config](#orgb769597)
-  [Building](#orgbf66ce2)
   -  [Prerequisites](#org727b3da)
   -  [Installation](#orgdd31739)
-  [Usage](#org4a9b3c6)
-  [Contributing](#org12345)

<a id="org4ab08b6"></a>

# About

Elkowar&rsquo;s Wacky Widgets is a standalone Widget System made in rust to add AwesomeWM like widgets to any WM

<a id="org581ca61"></a>

# Configuration

Eww&rsquo;s configuration should be placed in `~/.config/eww/eww.xml` and any `scss` styles you want to add should be put into `~/.config/eww/eww.scss`.

<a id="orgb769597"></a>

## Example Config

```xml
<eww>
  <definitions>
    <def name="test">
      <box orientation="v">
        {{foo}}
        <button onclick='notify-send "that hurt,..."'>
            click me if you dare :&lt;
          </button>
        <box>
          {{ree}}
          <scale min="0" max="100" value="50" onchange="notify-send {}"/>
        </box>
      </box>
    </def>
  </definitions>

  <variables>
    <var name="foo">test</var>
  </variables>


  <windows>
    <window name="main_window">
      <size x="100" y="200" />
      <pos x="100" y="200" />
      <widget>
        <test ree="test" />
      </widget>
    </window>
  </windows>
</eww>
```

<a id="orgbf66ce2"></a>

# Building

<a id="org727b3da"></a>

## Prerequisites

-  rustc
-  cargo (nightly toolchain)

Rather than with your system package manager, I recommend installing it using [rustup](https://rustup.rs/),
as this makes it easy to use the nightly toolchain, which is necessary to build eww.

<a id="orgdd31739"></a>

## Installation

Build the Binary using -:

    $ git clone https://github.com/Elkowar/eww.git
    $ cd eww
    $ cargo build --release

then copy the built binary from `./target/release` to anywhere in `$PATH` (example - `~/.local/bin`)

<a id="org4a9b3c6"></a>

# Usage

Create a Config and then just do `eww`!

<a id="org12345"></a>

# Contributing

If you wan't to contribute, like adding new widgets, features or subcommands, you should definitly do so.

Fork this repo install dependencies ([Prerequisites](#org727b3da)) smash your head on the keyboard from frustration and open a pull request, once you're finished.
