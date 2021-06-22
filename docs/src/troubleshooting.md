# Troubleshooting

Here you will find help if something doesn't work, if the issue isn't listed here please [open an issue on the GitHub repo.](https://github.com/elkowar/eww/issues)

## Eww does not compile

1. Make sure that you are compiling eww using the nightly rust toolchain.
2. Make sure you have all the necessary dependencies. If there are compile-errors, the compiler will tell you what you're missing.

## Eww does not work on wayland

1. Make sure you compiled eww with the `--no-default-features --features=wayland` flags.
2. Make sure that you're not trying to use X11-specific features (these are (hopefully) explicitly specified as such in the documentation).

## My scss isn't being loaded!

1. You have not created a scss file
2. The scss file isn't called correctly. (it should be called `eww.scss` in the `$HOME/.config/eww` folder)
3. The scss file isn't placed in the correct location (check above)

If none of these fixed your problem [open an issue on the GitHub repo](https://github.com/elkowar/eww/issues), or check the [GTK-Debugger](working_with_gtk.md#gtk-debugger).

## Eww can't find my configuration file!

1. It's incorrectly named or it's in the wrong place (it should be called `eww.xml` in the `$HOME/.config/eww` folder)
2. You haven't started eww correctly or you started it wrong.

## Something isn't styled correctly!

1. You have mistyped the CSS class.
2. Check the [GTK-Debugger](working_with_gtk.md#gtk-debugger)

## General issues

You should try the following things, before opening a issue or doing more specialized troubleshooting:

-   Kill the eww daemon by running `eww kill` and restart it with `eww --debug daemon` to get additional log output.
-   Now you can take a look at the logs by running `eww logs`.
-   use `eww state`, to see the state of all variables
-   use `eww debug`, to see the xml of your widget and other information
-   update to the latest eww version
-   sometimes hot reloading doesn't work. restart the widget in that case

If you're experiencing issues printing variables, try to print them in quotes, so e.g.

```
onchange="notify-send '{}'"
```

Remember, if your issue isn't listed here, [open an issue on the GitHub repo](https://github.com/elkowar/eww/issues).
