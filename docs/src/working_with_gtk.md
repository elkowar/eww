# GTK

## Gtk-theming

Eww is styled in GTK CSS.
To make theming even easier, it makes use of `scss` and then compiles that into CSS for you.
If you don't know any way to style something check out the [GTK CSS Overview wiki](https://developer.gnome.org/gtk3/stable/chap-css-overview.html),
the [GTK CSS Properties Overview wiki ](https://developer.gnome.org/gtk3/stable/chap-css-properties.html),
or check the [GTK-Debugger](#gtk-debugger).

If you have **NO** clue about how to do CSS, check out some online guides or tutorials.

SCSS is _very_ close to CSS so if you know CSS you'll have no problem learning SCSS.

## GTK-Debugger

The debugger can be used for **a lot** of things. Especially if something doesn't work or isn't styled right. to enable it launch your eww daemon with

```bash
GTK_DEBUG=interactive ./eww daemon
```

or in fish

```bash
env GTK_DEBUG=interactive ./eww daemon
```

If a style or something similar doesn't work you can click on the icon in the top left icon to select the thing that isn't being styled or isn't being styled correctly.

Then you can click on the drop down menu in the top right corner and select CSS Nodes, here you will see everything about styling it, CSS Properties and how it's structured.
