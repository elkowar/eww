# EWW - Elkowar's Wacky Widgets

## configuration

Eww's configuration should be placed in `~/.config/eww/eww.conf`.
any scss styles you want to add should be put into `~/.config/eww/eww.scss`.

### Example configuration

```hocon
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
```
