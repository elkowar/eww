# Table of Contents

-  [About](#org4ab08b6)
-  [Configuration](#org581ca61)
   -  [Example Config](#orgb769597)
-  [Building](#orgbf66ce2)
   -  [Prerequisites](#org727b3da)
   -  [Installation](#orgdd31739)
-  [Usage](#org4a9b3c6)
-  [Troubleshooting](#something)
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

<a id="something"></a>

# Troubleshooting

If you experience any issues, the following things should be tried:

- Try killing the eww daemon with `eww kill` and run again
- If you're running with `-d`, run without `-d` to see output 
- use `eww state`, to see the state of all variables
- use `eww debug`, to see the xml of your widget and other information
- update to the latest eww version
- sometimes hot reloading doesn't work. restart the widget in that case

If you're experiencing issues, printing variables try to print them in quotes, so e.g.
```
onchange="notify-send '{}'"
```

<a id="org12345"></a>

# Contributing

If you want to contribute, like adding new widgets, features or subcommands, you should definitly do so.

## Steps
1. Fork this repo
2. install dependencies ([Prerequisites](#org727b3da))
3. smash your head against the keyboard from frustration (coding is hard)
4. open a pull request once you're finished.
