# Writing your eww configuration

(For a list of all built-in widgets (i.e. `box`, `label`, `button`),  see [Widget Documentation](widgets.md).)\
Eww is configured using its own language called `yuck`.
Using yuck, you declare the structure and content of your widgets, the geometry, position, and behavior of any windows,
as well as any state and data that will be used in your widgets.
Yuck is based around S-expressions, which you may know from lisp-like languages.
If you're using vim, you can make use of [yuck.vim](https://github.com/elkowar/yuck.vim) for editor support.
If you're using VSCode, you can get syntax highlighting and formatting from [yuck-vscode](https://marketplace.visualstudio.com/items?itemName=eww-yuck.yuck).
It is also recommended to use [parinfer](https://shaunlebron.github.io/parinfer/),
which makes working with S-expressions delightfully easy!

Additionally, any styles are defined in CSS or SCSS (which is mostly just slightly improved CSS syntax).
While eww supports a significant portion of the CSS you know from the web,
not everything is supported, as eww relies on GTK's own CSS engine.
Notably, some animation features are unsupported,
as well as most layout-related CSS properties such as flexbox, `float`, absolute position or `width`/`height`.

To get started, you'll need to create two files: `eww.yuck` and `eww.scss` (or `eww.css`, if you prefer).
These files must be placed under `$XDG_CONFIG_HOME/eww` (this is most likely `~/.config/eww`).

Now that those files are created, you can start writing your first widget!

## Creating your first window

Firstly, you will need to create a top-level window. Here, you configure things such as the name, position, geometry, and content of your window.

Let's look at an example window definition:

```lisp
(defwindow example
           :monitor 0
           :geometry (geometry :x "0%"
                               :y "20px"
                               :width "90%"
                               :height "30px"
                               :anchor "top center")
           :stacking "fg"
           :reserve (struts :distance "40px" :side "top")
           :windowtype "dock"
           :wm-ignore false
  "example content")
```

Here, we are defining a window named `example`, which we then define a set of properties for. Additionally, we set the content of the window to be the text `"example content"`.

You can now open your first window by running `eww open example`! Glorious!

### `defwindow`-properties

|   Property | Description                                                  |
| ---------: | ------------------------------------------------------------ |
|  `monitor` | Which monitor this window should be displayed on. See below for details.|
| `geometry` | Geometry of the window.  |


**`monitor`-property**

This field can be:

- the string `<primary>`, in which case eww tries to identify the primary display (which may fail, especially on wayland)
- an integer, declaring the monitor index
- the name of the monitor
- a string containing a JSON-array of monitor matchers, such as: `'["<primary>", "HDMI-A-1", "PHL 345B1C", 0]'`. Eww will try to find a match in order, allowing you to specify fallbacks.


**`geometry`-properties**

| Property          | Description |
| -----------------:| ------------------------------------------------------------ |
|          `x`, `y` | Position of the window. Values may be provided in `px` or `%`. Will be relative to `anchor`. |
| `width`, `height` | Width and height of the window. Values may be provided in `px` or `%`. |
|          `anchor` | Anchor-point of the window. Either `center` or combinations of `top`, `center`, `bottom` and `left`, `center`, `right`. |

<br/>
Depending on if you are using X11 or Wayland, some additional properties exist:

#### X11

|     Property | Description                                                  |
| -----------: | ------------------------------------------------------------ |
|   `stacking` | Where the window should appear in the stack. Possible values: `fg`, `bg`. |
|  `wm-ignore` | Whether the window manager should ignore this window. This is useful for dashboard-style widgets that don't need to interact with other windows at all. Note that this makes some of the other properties not have any effect. Either `true` or `false`. |
|    `reserve` | Specify how the window manager should make space for your window. This is useful for bars, which should not overlap any other windows. |
| `windowtype` | Specify what type of window this is. This will be used by your window manager to determine how it should handle your window. Possible values: `normal`, `dock`, `toolbar`, `dialog`, `desktop`. Default: `dock` if `reserve` is specified, `normal` otherwise. |

#### Wayland

|    Property | Description                                                                                                                                                            |
| ----------: |------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
|  `stacking` | Where the window should appear in the stack. Possible values: `fg`, `bg`, `overlay`, `bottom`.                                                                         |
| `exclusive` | Whether the compositor should reserve space for the window automatically. Either `true` or `false`. If `true` `:anchor` has to include `center`.                       |
| `focusable` | Whether the window should be able to be focused. This is necessary for any widgets that use the keyboard to work. Possible values: `none`, `exclusive` and `ondemand`. |
| `namespace` | Set the wayland layersurface namespace eww uses. Accepts a `string` value.                                                                                             |



## Your first widget

While our bar is already looking great, it's a bit boring. Thus, let's add some actual content!

```lisp
(defwidget greeter [?text name]
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

We are creating a widget named `greeter`. This widget takes two attributes, called `text` and `name`.
The declaration `?text` specifies that the `text`-attribute is optional, and can thus be left out. In that case,
its value will be the empty string `""`.
The `name` attribute _must_ be provided.

Now we declare the body of our widget. We make use of a `box`, which we set a couple attributes of.

We need this `box`, as a widget definition can only ever contain a single widget - otherwise,
eww would not know if it should align them vertically or horizontally, how it should space them, and so on.
Thus, we wrap multiple children in a `box`.
This box then contains a reference to the provided attribute `text`, as well as a button.
In that button's `onclick` attribute, we refer to the provided `name` using string-interpolation syntax: `"${name}"`.
This allows us to easily refer to any variables within strings.
In fact, there is a lot more you can do within `${...}` - more on that in the chapter about the [expression language](expression_language.md).

To then use our widget, we call it just like we would use any other built-in widget and provide the required attributes.

As you may have noticed, we are using a couple predefined widgets here. These are all listed and explained in the [widgets chapter](widgets.md).


### Rendering children in your widgets
As your configuration grows, you might want to improve the structure of your config by factoring out functionality into basic reusable widgets.
Eww allows you to create custom wrapper widgets that can themselves take children, just like some of the built-in widgets like `box` or `button` can.
For this, use the `children` placeholder:
```lisp
(defwidget labeled-container [name]
  (box :class "container"
    name
    (children)))
```
Now you can use this widget as expected:
```lisp
(labeled-container :name "foo"
  (button :onclick "notify-send hey ho"
    "click me"))
```

You can also create more complex structure by referring to specific children with the `nth`-attribute:
```lisp
(defwidget two-boxes []
  (box
    (box :class "first" (children :nth 0))
    (box :class "second" (children :nth 1))))
```

## Adding dynamic content

Now that you feel sufficiently greeted by your bar, you may realize that showing data like the time and date might be even more useful than having a button that greets you.

To implement dynamic content in your widgets, you make use of _variables_.

These user-defined variables are globally available from all of your widgets. Whenever the variable changes, the value in the widget will update!

There are four different types of variables: basic, polling, listening, and a set of builtin "magic" variables.

**Basic variables (`defvar`)**

```lisp
(defvar foo "initial value")
```

This is the simplest type of variable.
Basic variables don't ever change automatically.
Instead, you explicitly update them by calling eww like so: `eww update foo="new value"`.

This is useful if you have values that change very rarely, or may change as a result of some external script you wrote.
They may also be useful to have buttons within eww change what is shown within your widget, by setting attributes like `onclick` to run `eww update`.

**Polling variables (`defpoll`)**

```lisp
(defvar time-visible false)   ; for :run-while property of below variable
                              ; when this turns true, the polling starts and
                              ; var gets updated with given interval

(defpoll time :interval "1s"
              :initial "initial-value"  ; optional, defaults to poll at startup
              :run-while time-visible   ; optional, defaults to 'true'
  `date +%H:%M:%S`)
```

A polling variable is a variable which runs a provided shell-script repeatedly, in a given interval.

This may be the most commonly used type of variable.
They are useful to access any quickly retrieved value repeatedly,
and thus are the perfect choice for showing your time, date, as well as other bits of information such as pending package updates, weather, and battery level.

You can also specify an initial-value. This should prevent eww from waiting for the result of a given command during startup, thus
making the startup time faster.

To externally update a polling variable, `eww update` can be used like with basic variables to assign a value.
You can also call `eww poll` to poll the variable outside of its usual interval, or even while it isn't running at all.

**Listening variables (`deflisten`)**

```lisp
(deflisten foo :initial "whatever"
  `tail -F /tmp/some_file`)
```

Listening variables might be the most confusing of the bunch.
A listening variable runs a script once, and reads its output continously.
Whenever the script outputs a new line, the value will be updated to that new line.
In the example given above, the value of `foo` will start out as `"whatever"`, and will change whenever a new line is appended to `/tmp/some_file`.

These are particularly useful when you want to apply changes instantaneously when an operation happens if you have a script
that can monitor some value on its own. Volume, brightness, workspaces that get added/removed at runtime,
monitoring currently focused desktop/tag, etc. are the most common usecases of this type of variable.
These are particularly efficient and should be preffered if possible.

For example, the command `xprop -spy -root _NET_CURRENT_DESKTOP` writes the currently focused desktop whenever it changes.
Another example usecase is monitoring the currently playing song with playerctl: `playerctl --follow metadata --format {{title}}`.

**Built-in "magic" variables**

In addition to defining your own variables, eww provides some values for you to use out of the box.
These include values such as your CPU and RAM usage.
These mostly contain their data as JSON, which you can then get using the [json access syntax](expression_language.md).
All available magic variables are listed [here](magic-vars.md).

## Dynamically generated widgets with `literal`

In some cases, you want to not only change the text,
value, or color of a widget dynamically, but instead want to generate an entire widget structure dynamically.
This is necessary if you want to display lists of things (for example notifications)
where the amount is not necessarily known,
or if you want to change the widget structure in some other, more complex way.

For this, you can make use of one of eww's most powerful features: the `literal` widget.

```lisp
(defvar variable_containing_yuck
  "(box (button 'foo') (button 'bar'))")

; Then, inside your widget, use:
(literal :content variable_containing_yuck)
```

Here, you specify the content of your literal by providing it a string (most likely stored in a variable) which contains a single yuck widget tree.
Eww then reads the provided value and renders the resulting widget. Whenever it changes, the widget will be rerendered.

Note that this is not all that efficient. Make sure to only use `literal` when necessary!

## Using window arguments and IDs

In some cases you may want to use the same window configuration for multiple widgets, e.g. for multiple windows. This is where arguments and ids come in.

### Window ID

Firstly let us start off with ids. An id can be specified in the `open` command
with `--id`, by default the id will be set to the name of the window
configuration. These ids allow you to spawn multiple of the same windows. So
for example you can do:

```bash
eww open my_bar --screen 0 --id primary
eww open my_bar --screen 1 --id secondary
```

When using `open-many` you can follow the structure below. Again if no id is
given, the id will default to the name of the window configuration.

```bash
eww open-many my_config:primary my_config:secondary
```

You may notice with this we didn't set `screen`, this is set through the
`--arg` system, please see below for more information.

### Window Arguments

However this may not be enough and you want to have slight changes for each of
these bars, e.g. having a different class for 1080p displays vs 4k or having
spawning the window in a different size or location. This is where the
arguments come in.

Please note these arguments are **CONSTANT** and so cannot be update after the
window has been opened.

Defining arguments in a window is the exact same as in a widget so you can
have:

```lisp
(defwindow my_bar [arg1 ?arg2]
          :geometry (geometry
                       :x      "0%"
                       :y      "6px"
                       :width  "100%"
                       :height { arg1 == "small" ? "30px" : "40px" }
                       :anchor "top center")
          :stacking   "bg"
          :windowtype "dock"
          :reserve    (struts :distance "50px" :side "top")
    (my_widget :arg2 arg2))
```

Here we have two arguments, `arg1` and `arg2` (an optional parameter).

Once we have these parameters, when opening a new window, we must specify them
(unless they are optional, like `arg2`), but how? Well, we use the `--arg`
option when running the `open` command:

```bash
eww open my_bar --id primary --arg arg1=some_value --arg arg2=another_value
```

With the `open-many` it looks like this:

```bash
# Please note that `--arg` option must be given after all the windows names
eww open-many my_bar:primary --arg primary:arg1=some_value --arg primary:arg2=another_value
```

Using this method you can define `screen`, `anchor`, `pos`, `size` inside the
args for each window and it will act like giving `--screen`, `--anchor` etc. in
the `open` command.

So, now you know the basics, I shall introduce you to some of these "special"
parameters, which are set slightly differently. However these can all be
overridden by the `--arg` option.

- `id` - If `id` is included in the argument list, it will be set to the id
  specified by `--id` or will be set to the name of the config. This can be
  used when closing the current window through eww commands.
- `screen` - If `screen` is specified it will be set to the value given by
  `--screen`, so you can use this in other widgets to access screen specific
  information.

### Further insight into args in `open-many`

Now due to the system behind processing the `open-many` `--arg` option you
don't have to specify an id for each argument. If you do not, that argument
will be applied across all windows e.g.

```bash
eww open-many my_bar:primary my_bar:secondary --arg gui_size="small"
```

This will mean the config is the same throughout the bars.

Furthermore if you didn't specify an id for the window, you can still set args
specifically for that window - following the idea that the id will be set to
the window configuration if not given - by just using the name of the window
configuration e.g.

```bash
eww open-many my_primary_bar --arg my_primary_bar:screen=0
```

## Generating a list of widgets from JSON using `for`

If you want to display a list of values, you can use the `for`-Element to fill a container with a list of elements generated from a JSON-array.
```lisp
(defvar my-json "[1, 2, 3]")

; Then, inside your widget, you can use
(box
  (for entry in my-json
    (button :onclick "notify-send 'click' 'button ${entry}'"
      entry)))
```

This can be useful in many situations, for example when generating a workspace list from a JSON representation of your workspaces.
In many cases, this can be used instead of `literal`, and should most likely be preferred in those cases.

To see how to declare and use more advanced data structures, check out the [data structures example](/examples/data-structures/eww.yuck).

## Splitting up your configuration

As time passes, your configuration might grow larger and larger. Luckily, you can easily split up your configuration into multiple files!

There are two options to achieve this:

### Using `include`

```lisp
(include "./path/to/your/file.yuck")
```

A single yuck file may import the contents of any other yuck file. For this, make use of the `include` directive.

### Using a separate eww configuration directory

If you want to separate different widgets even further, you can create a new eww config folder anywhere else.
Then, you can tell eww to use that configuration directory by passing _every_ command the `--config /path/to/your/config/dir` flag.
Make sure to actually include this in all your `eww` calls, including `eww kill`, `eww logs`, etc.
This launches a separate instance of the eww daemon that has separate logs and state from your main eww configuration.
