+++
title = "Configuration"
slug = "The basics of how to configure eww"
weight = 1
+++

## Configuration

For specific built in widgets `<box>, <text>, <slider>, etc` see [Widget Documentation](@/main/widgets.md)

### Placing the configuration file

The configuration file and the scss file should lay in `$XDG_CONFIG_HOME/eww` (or, if unset, `$HOME/.config/eww`). The XML file should be named `eww.xml` and the scss should be named `eww.scss`
So the directory structure should look like this:
```
$HOME
└──.config
   └──eww
      ├──eww.xml
      └──eww.scss
```

### Variables

If you create a `<var>` or a `<script-var>`, you can reference them in your `<box>` by doing `{{var}}`. Where `var` is your variable name.

#### The `<var>` tag
Allows you to repeat the same text multiple times through  without retyping it multiple times.

Example: This will define a variable named `banana`, with the default value "I like bananas."
```xml
<variables>
    <var name="banana">I like bananas.</var>
</variables>
```
You can then reference it in your widgets by doing:

```xml
<box>
    {{banana}}
</box>
```

To change the value of the variable, and thus change the UI, you can run `eww update banana "I like apples"`

#### The `<script-var>` tag

Allows you to create a script that eww runs.
Useful for creating volume sliders or anything similar.

Example:
```xml
<variables>
    <script-var name="date" interval="5s">
    date +%H:%M
    </script-var>
</variables>
```

and then reference it by doing:
```xml
<box>
    {{date}}
</box>
```

The `interval="5s"` part says how long time it should take before Eww runs the command again.
Here are the available times you can set:

| Shortened | Full name   |
|-----------|-------------|
| ms        | Miliseconds |
| s         | Seconds     |
| m         | Minutes     |
| h         | Hours       |


#### Tail
If you don't want a set interval and instead want it to tail (run the script when it detects a change is present) you can simply remove the `interval="5s"` so it becomes:
```xml
<variables>
    <script-var name="date">
    date +%H:%M
    </script-var>
</variables>
```

### The `<definitions>` block
In here you whole widget will be made, and you can also create your own widgets. Check [Widget Documentation](@/main/widgets.md) for pre-defined widgets.

#### Custom widgets

Let's get a small config and break it down.

```xml
<definitions>
    <def name="clock">
        <box>
            The time is: {{my_time}} currently.
        </box>
    </def>
    <def name="main">
        <box>
            <clock my_time="{{date}}"/>
        </box>
    </def>
</definitions>

<variables>
    <script-var name="date">
        date
    </script-var>
</variables>
```
That's a long config just for a custom widget. But let's break it down and try to understand it.

This part:
```xml
<def name="clock">
    <box>
	The time is: {{my_time}} currently.
    </box>
</def>
```
Is the custom widget. As we can see by the
```xml
<def name="clock">
```
the widget is called `clock.`Or referenced `<clock>`
The `{{my_time}}` is the value we assign to be well, our time. You can actually set to be anything, it doesn't have to be a time. You can compare it to `value=""`

So if we look at:
```xml
<def name="main">
    <box>
	<clock my_time="{{date}}"/>
    </box>
</def>
```
we can see that we assign `{{my_time}}` to be `{{date}}` and if we look at
```xml
<script-var name="date">
    date
</script-var>
```
we can see that `{{date}}` is simply running the `date` command.

It doesn't have to be `{{my_time}}` either, it can be anything.
```xml
<def name="clock">
    <box>
	The time is: {{very_long_list_of_animals}} currently.
    </box>
</def>
```
is valid.

To use that it would look like this:
```xml
<def name="main">
    <box>
	<clock very_long_list_of_animals="{{date}}"/>
    </box>
</def>
```
### The `<windows>` block {#windows-block}

This is the part the Eww reads and loads. The `<windows>` config should look something like this:

```xml
<windows>
    <window name="main_window" stacking="fg">
      <size x="300" y="300" />
      <pos x="0" y="500" />
      <widget>
	<main/>
      </widget>
    </window>
</windows>
```
`<window name="main_window">` is the part that eww runs when you start it. In this example you would run eww by doing:
```bash
./eww open main_window
```
but if renamed the `<window>` to be `<window name="apple">` we would run eww by doing:
```bash
./eww open apple
```

The `stacking="fg"` says where the widget will be stacked. Possible values here are `foreground`, `fg`, `background` and `bg`.
`foreground` or `fg` *always* stays above windows.
`background` or `bg` *always* stays behind windows. So it will stay on your desktop.

If you were to remove the `stacking="fg"` it would default it to `fg`.

You can also have multiple windows in one document by  doing:

```xml
<windows>
    <window name="main_window">
        <size x="300" y="300" />
        <pos x="0" y="500" />
        <widget>
            <main/>
        </widget>
    </window>
    <window name="main_window2">
        <size x="400" y="600"/>
        <pos x="0" y="0"/>
        <widget>
            <main2/>
        </widget>
    </window>
</windows>
```
---

- `<size>` sets x-y size of the widget.
- `<pos>` sets x-y position of the widget.
- `<widget>` is the part which you say which `<def>` eww should run. So if we take the example config from before:
```xml
<definitions>
    <def name="clock">
        <box>
            The time is: {{my_time}} currently.
        </box>
    </def>
    <def name="main">
        <box>
            <clock my_time="{{date}}"/>
        </box>
    </def>
</definitions>
```
and then look at
```xml
<widget>
    <main/>
</widget>
```
we will see that eww will run `<def name="main">` and not `<def name="clock">`.
