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

### Config structure

Your config structure should look like this:
```xml
<eww>
    <definitions>
		<!-- Put your <file>'s in here -->
	</definitions>
    
	<definitions>
		<!-- Put your <def>'s in here -->
	</definitions>

	<variables>
		<!-- Put your <script-var> and <var>'s in here -->
	</variables>

	<windows>
		<!-- Put your window blocks here -->
	</windows>
</eww>
```
See 
[The `<includes>` block](#the-includes-block),
[The `<definitons>` block](#the-definitions-block),
[Variables](#variables) and the
[The `<windows>` block](#the-windows-block).

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
### The `<includes>` block
Here you can include other config files so that they are merged together at startup. Currently namespaced variables are not supported so be careful when reusing code.

```xml
<includes>
  <file path="./something.xml"/>
  <file path="./somethingelse.xml"/>
</includes>
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

All different windows you might want to use are defined in the `<windows>` block.
The `<windows>` config should look something like this:

```xml
<windows>
    <window name="main_window" stacking="fg" focusable="false" screen="1">
        <geometry anchor="top left" x="300px" y="50%" width="25%" height="20px"/>
        <widget>
            <main/>
        </widget>
    </window>
</windows>
```

The window block contains multiple elements to configure the window.
- `<geometry>` is used to specify the position and size of the window.
- `<widget>` will contain the widget that is shown in the window.

There are a couple things you can optionally configure on the window itself:
- `stacking`: stacking describes if the window will be shown in the foreground (in front of other windows)
  or in the background (behind other windows).  
  Possible values: `"fg"`, `"bg"`. Default: `"fg"`
- `focusable`: whether the window should be focusable by the windowmanager.
  This is necessary for things like text-input-fields to work properly.  
  Possible values: `"true"`, `"false"`. Default: `"false"`
- `screen`: Specifies on which display to show the window in a multi-monitor setup.  
  This can be any number, representing the index of your monitor.
