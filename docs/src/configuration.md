# Writing your eww configuration

(For a list of all built in widgets (i.e. `box`, `text`, `slider`)  see [Widget Documentation](widgets.md))

Eww is configured using its own language called `yuck`. using yuck, you declare the structure and content of your widgets, the geometry, position and behavior of any windows, as well as any state and data that will be used in your widgets. Yuck is based around S-expressions, which you may know from lisp-like languages. If you're using vim, you can make use of [yuck.vim](https://github.com/elkowar/yuck.vim) for editor support. It is also recommended to use [parinfer](https://shaunlebron.github.io/parinfer/), which makes working with s-expressions delightfully easy!

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

You can now open your first window by running `eww open example`! Glorious!

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



## Your first widget

While our bar is already looking great, it's a bit boring. Thus, let's add some actual content!

```lisp
(defwidget greeter [text name]
  (box :orientation "horizontal"
       :halign "center"
    text
    (button :onclick "notify-send 'Hello' 'Hello, ${name}'"
      "Greet")))
```

To show this, let's replace the text in our window definition with a call to this new widget:

```lisp
(defwindow example
           ; ... values omitted
  (greeter :text "Say hello!"
           :name "Tim"))
```

There is a lot going on here, so let's step through this.

We are creating a widget named `greeter`. This widget takes two attributes, called `text` and `name`, which must be set when the widget is used.

Now, we declare the body of our widget. We make use of a `box`, which we set a couple attributes of. This box then contains a reference to the provided attribute `text`, as well as a button. In that buttons `onclick` attribute, we make refer to the provided `name` using string-interpolation syntax: `"${name}"`.  This allows us to easily refer to any variables within strings. In fact, there is a lot more you can do withing `${...}` - more on that in the chapter about the [expression language](expression_language.md). 

To then use our widget, we call it just like we would use any other built-in widget, and provide the required attributes.

As you may have noticed, we are using a couple predefined widgets here. These are all listed and explained in the [widgets chapter](widgets.md).



## Adding dynamic content

Now that you feel sufficiently greeted by your bar, you may realize that showing data like the time and date might be even more useful than having a button that greets you.

To implement dynamic content in your widgets you make use of _variables_.

These user-defined variables are globally available from all of your widgets. Whenever the variable changes, the value in the widget will update!

There are four different types of variables: basic, polling, listening, and a set of builtin "magic" variables.

**Basic variables (`defvar`)**

```lisp
(defvar foo "initial value")
```

This is the simplest type of variable. Basic variables don't ever change automatically. Instead, you explicitly update them by calling eww like so: `eww update foo="new value"`.

This is useful if you have values that change very rarely, or may change as a result of some external script you wrote. They may also be useful to have buttons within eww change what is shown within your widget, by setting attributes like `onclick` to run `eww update`.

**Polling variables (`defpoll`)**

```lisp
(defpoll time :interval "1s"
              :timeout "0.1s" ; setting timeout is optional
  `date +%H:%M:%S`)
```

A polling variable is a variable which runs a provided shell-script repeatedly, in a given interval.

This may be the most commonly used type of variable. They are useful to access any quickly retrieved value repeatedly, and thus are the perfect choice for showing your time, date, as well as other bits of information such as your volume.

Optionally, you can specify a timeout, after which the provided script will be aborted. This helps to avoid accidentally launching thousands of never-ending processes on your system.

**Listening variables (`deflisten`)**

```lisp
(deflisten foo :initial "whatever"
  `tail -F /tmp/some_file`)
```

Listening variables might be the most confusing of the bunch.  A listening variable runs a script once, and reads its output continously. Whenever the script outputs a new line, the value will be updated to that new line. In the example given above, the value of `foo` will start out as `"whatever"`, and will change whenever a new line is appended to `/tmp/some_file`.

These are particularly useful if you have a script that can monitor some value on its own. For example, the command `xprop -spy -root _NET_CURRENT_DESKTOP` writes the currently focused desktop whenever it changes. This can be used to implement a workspace widget for a bar, for example. Another example usecase is monitoring the currently playing song with playerctl: `playerctl --follow metadata --format {{title}}`.

**Built-in "magic" variables**

In addition to definition your own variables, eww provides some values for you to use out of the box. These include values such as your CPU and RAM usage. These mostly contain their data as JSON, which you can then use using the [json access syntax](expression_language.md). All available magic variables are listed [here](magic-vars.md).

