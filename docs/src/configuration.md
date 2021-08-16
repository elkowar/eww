# Writing your eww configuration

(For a list of all built in widgets (i.e. `box`, `text`, `slider`)  see [Widget Documentation](widgets.md))

Eww is configured using it's own language called `yuck`. using yuck, you declare the structure and content of your widgets, the geometry, position and behavior of any windows, as well as any state and data that will be used in your widgets. Yuck is based around S-expressions, which you may know from lisp-like languages. If you're using vim, you can make use of [yuck.vim](https://github.com/elkowar/yuck.vim) for editor support. It is also recommended to use [parinfer](https://shaunlebron.github.io/parinfer/), which makes working with s-expressions delightfully easy!

Additionally, any styles are defined in scss (which is mostly just slightly improved CSS syntax). While eww supports a significant portion of the CSS you know from the web, not everything is supported, as eww relies on GTKs own CSS engine. Notably, some animation features are unsupported, as well as most layout-related CSS properties such as flexbox, `float`, absolute position or `width`/`height`.

To get started, you'll need to create two files: `eww.yuck` and `eww.scss`. These files must be placed under `$XDG_CONFIG_HOME/eww` (this is most likely `~/.config/eww`).

Now that those files are created, you can start writing your first widget!

## Creating your first window

Firstly, you will need to create a top-level window. Here, you configure things such as the name, position, geometry and content of your window.

Let's look at an example window definition:

```lisp
(defwindow example
           :monitor 0
           :geometry (geometry :x 0 :y 0 :width "90%" :height "30px")
           :anchor "top center"
           :stacking "fg"
           :reserve (struts :distance "40px" :side "top")
           :windowtype "dock"
           :wm-ignore false
  "example content")
```

Here, we are defining a window named `example`, which we then set a set of properties for. Additionally, we set the content of the window to be the text `"example content"`.

### `defwindow`-Properties

|   Property | Description                                                  |
| ---------: | ------------------------------------------------------------ |
|  `monitor` | which monitor this window should be displayed on             |
| `geometry` | Position and size of the window. Values may be provided in `px` or relative to the screen size. |
|   `anchor` | What side of the screen the window should be anchored to. Either `center` or combinations of `top`, `center`, `bottom` and `left`, `center`, `right` |

Depending on if you are using X11 or wayland, some additional properties exist:

#### x11

|     Property | Description                                                  |
| -----------: | ------------------------------------------------------------ |
|   `stacking` | Where the window should appear in the stack. Possible values: `fg`, `bg`. |
|  `wm-ignore` | Whether the windowmanager should ignore this window. This is useful for dashboard-style widgets that don't need to interact with other windows at all. Note that this makes some of the other properties not have any effect. Either `true` or `false` |
|    `reserve` | Specify how the window-manager should make space for your window. This is useful for bars, which should not overlap any other windows. |
| `windowtype` | Specify what type of window this is. This will be used by your window manager to determine how it should handle your window. Possible values: `normal`, `dock`, `toolbar`, `dialog`. Default: `dock` if `reserve` is specified, `normal` otherwise. |

#### wayland

|    Property | Description                                                  |
| ----------: | ------------------------------------------------------------ |
|  `stacking` | Where the window should appear in the stack. Possible values: `fg`, `bg`, `overlay`, `bottom`. |
| `exclusive` | Whether the compositor should reserve space for the window automatically. |
| `focusable` | Whether the window should be able to be focused. This is necessary for any widgets that use the keyboard to work. |
