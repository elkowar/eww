# Migrating to yuck

Yuck is the new configuration syntax used by eww.
While the syntax has changed dramatically, the general structure of the configuration
has stayed mostly the same.

Most notably, the top-level blocks are now gone.
This means that `defvar`, `defwidget`, etc blocks no longer need to be in separate
sections of the file, but instead can be put wherever you need them.

Explaining the exact syntax of yuck would be significantly less effective than just
looking at an example, as the general syntax is very simple.

Thus, to get a feel for yuck, read through the [example configuration](./examples/eww-bar/eww.yuck).


Additionally, a couple smaller things have been changed.
The fields and structure of the `defwindow` block as been adjusted to better reflect
the options provided by the displayserver that is being used.
The major changes are:
- The `screen` field is now called `monitor`
- `reserve` and `geometry` are now structured slightly differently (see [here](./docs/src/configuration.md#creating-your-first-window))
To see how exactly the configuration now looks, check the [respective documentation](./docs/src/configuration.md#creating-your-first-window)


## Automatically converting your configuration

A couple _amazing_ people have started to work on an [automatic converter](https://github.com/undefinedDarkness/ewwxml) that can turn your
old eww.xml into the new yuck format!
