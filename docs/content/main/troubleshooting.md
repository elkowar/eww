+++
title = "Troubleshooting"
slug = "troubleshooting"
weight = 4
+++

## Troubleshooting

Here you will find help if something doesn't work, if the issue isn't listed here please [open an issue on the GitHub repo.](https://github.com/elkowar/eww/issues)

### My scss isn't being loaded!

1. You have not created a scss file
2. The scss file isn't called correctly. (it should be called `eww.scss` in the `$HOME/.config/eww` folder)
3. The scss file isn't placed in the correct location (check above)
4. You have used two (or more) semi-colons (;;) instead of one (;) at the end of a line.

If none of these fixed your problem [open an issue on the GitHub repo.](https://github.com/elkowar/eww/issues) Or check the [GTK-Debugger](#gtk-debugger).

### Eww can't find my configuration file!

1. It's incorrectly named or it's in the wrong place  (it should be called `eww.xml` in the `$HOME/.config/eww` folder)
2. You haven't started eww correctly or you started it wrong. (See [Starting  Eww](starting-eww))

### Something isn't styled correctly!

1. You have mistyped the CSS class.
2. Check the [GTK-Debugger](#gtk-debugger)

### General issues

You should try the following things, before opening a issue or doing more specialized troubleshooting: 

- Try killing the eww daemon with `eww kill` and run again
- If you're running with `-d`, run without `-d` to see output, or have a look at ~/.cache/eww.log
- use `eww state`, to see the state of all variables
- use `eww debug`, to see the xml of your widget and other information
- update to the latest eww version
- sometimes hot reloading doesn't work. restart the widget in that case

If you're experiencing issues printing variables, try to print them in quotes, so e.g.
```
onchange="notify-send '{}'"
```

Remember if your issue isn't listed here,  [open an issue on the GitHub repo](https://github.com/elkowar/eww/issues).
