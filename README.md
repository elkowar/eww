
# Table of Contents

*  [About](#org4ab08b6)
*  [Configuration](#org581ca61)
    *  [Example Config](#orgb769597)
*  [Building](#orgbf66ce2)
    *  [Prerequisites](#org727b3da)
    *  [Installation](#orgdd31739)
*  [Usage](#org4a9b3c6)



<a id="org4ab08b6"></a>

# About

Elkowar&rsquo;s Wacky Widgets is a standalone Widget System made in rust to add AwesomeWM like widgets to any WM


<a id="org581ca61"></a>

# Configuration

Eww&rsquo;s configuration should be placed in `~/.config/eww/eww.conf` and any `scss` styles you want to add should be put into `~/.config/eww/eww.scss`.


<a id="orgb769597"></a>

## Example Config

    widgets: {
        some_widget: {
            structure: {
                layout: {
                    class: "container",
                    children: [
                        { layout: {
                            orientation: "v"
                            children: [
                                { button: "brah" }
                            ]
                        } }
                        { layout: {
                            children: [
                                "$$date"
                                { button: "Some button" }
                                { slider: { value: "$$some_value", min: 0, max: 100, onchange: "notify-send 'changed' {}" } }
                                { slider: { value: "$$some_value", orientation: "h" } }
                                "hu"
                            ]
                        } }
                    ]
                }
            }
        },
        test: {
            structure: {
                some_widget: {
                    some_value: "$$ooph"
                }
            }
        },
        bar: {
            structure: {
                layout: {
                    children: [
                        { layout: { halign: left, children: "text" } }
                        { layout: { halign: center, hexpand: false, children: "$$date" }}
                        { layout: {
                            halign: end,
                            hexpand: false,
                            children: [
                                "$$date"
                                { slider: { value: "$$some_value", min: 0, max: 100, onchange: "notify-send 'changed' {}" } }
                                "$$date"
                            ]
                        } }
                        { label: { text: { run: "date", interval: 1s } } }
                    ]
                }
            }
        },
    }
    default_vars: {
        foo: 12
        date: "neverrrr"
    },
    windows: {
        main_window: {
            pos.x: 0
            pos.y: 1080
            size.x: 2560
            size.y: 29
            widget: {
                bar: {
                    some_value: "$$foo"
                }
            }
        }
    }


<a id="orgbf66ce2"></a>

# Building


<a id="org727b3da"></a>

## Prerequisites

-   rustc
-   cargo

Get them at <https://www.rust-lang.org/tools/install>


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

