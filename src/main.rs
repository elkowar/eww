#![feature(try_blocks)]
extern crate gio;
extern crate gtk;

use anyhow::{self, Result};
use gdk::*;
use gio::prelude::*;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow};
use std::{collections::HashMap, process::Command};

pub mod config;
pub mod widgets;

const CMD_STRING_PLACEHODLER: &str = "{}";

const EXAMPLE_CONFIG: &str = r#"{
    widgets: {
        some_widget: {
            structure: {
                layout_horizontal: {
                    class: "container",
                    children: [
                        "hi",
                        { button: { children: "click me you" } }
                        { slider: { value: 12, min: 0, max: 50, onchange: "notify-send 'changed' {}" } }
                        "hu"
                    ]
                }
            }
        }
    },
    windows: {
        main_window: {
            pos.x: 200
            pos.y: 1550
            size.x: 500
            size.y: 50
            widget: {
                some_widget: {}
            }
        }
    },

}"#;

fn main() -> Result<()> {
    let eww_config = config::EwwConfig::from_hocon(&config::parse_hocon(EXAMPLE_CONFIG)?)?;

    let application = Application::new(Some("de.elkowar.eww"), gio::ApplicationFlags::FLAGS_NONE)
        .expect("failed to initialize GTK application");

    let window_def = eww_config.windows()["main_window"].clone();

    application.connect_activate(move |app| {
        let app_window = ApplicationWindow::new(app);
        app_window.set_title("Eww");
        app_window.set_wmclass("noswallow", "noswallow");
        app_window.set_type_hint(gdk::WindowTypeHint::Dock);
        app_window.set_position(gtk::WindowPosition::Center);
        app_window.set_keep_above(true);
        app_window.set_default_size(window_def.size.0, window_def.size.1);
        app_window.set_visual(
            app_window
                .get_display()
                .get_default_screen()
                .get_rgba_visual()
                .or_else(|| {
                    app_window
                        .get_display()
                        .get_default_screen()
                        .get_system_visual()
                })
                .as_ref(),
        );

        app_window.fullscreen();

        let widget_state = WidgetState(HashMap::new());

        app_window.add(
            &element_to_gtk_widget(&eww_config.widgets(), &widget_state, &window_def.widget)
                .unwrap(),
        );

        app_window.show_all();

        let window = app_window.get_window().unwrap();

        window.set_override_redirect(true);
        window.move_(window_def.position.0, window_def.position.1);
        window.show();
        window.raise();
    });

    application.run(&[]);
    Ok(())
}

fn element_to_gtk_widget(
    widget_definitions: &HashMap<String, config::WidgetDefinition>,
    widget_state: &WidgetState,
    element: &config::ElementUse,
) -> Option<gtk::Widget> {
    match element {
        config::ElementUse::Text(text) => Some(gtk::Label::new(Some(&text)).upcast()),
        config::ElementUse::Widget(widget) => {
            let gtk_widget =
                widget_use_to_gtk_container(widget_definitions, widget_state, &widget).or(
                    widget_use_to_gtk_widget(widget_definitions, widget_state, &widget),
                )?;
            if let Some(css_class) = widget
                .attrs
                .get("class")
                .and_then(config::AttrValue::as_string)
            {
                gtk_widget.get_style_context().add_class(css_class);
            }

            Some(gtk_widget)
        }
    }
}

fn widget_use_to_gtk_container(
    widget_definitions: &HashMap<String, config::WidgetDefinition>,
    widget_state: &WidgetState,
    widget: &config::WidgetUse,
) -> Option<gtk::Widget> {
    let container_widget: gtk::Container = match widget.name.as_str() {
        "layout_horizontal" => gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast(),
        "button" => gtk::Button::new().upcast(),
        _ => return None,
    };

    for child in &widget.children {
        container_widget.add(&element_to_gtk_widget(
            widget_definitions,
            widget_state,
            child,
        )?);
    }
    Some(container_widget.upcast())
}

fn widget_use_to_gtk_widget(
    widget_definitions: &HashMap<String, config::WidgetDefinition>,
    state: &WidgetState,
    widget: &config::WidgetUse,
) -> Option<gtk::Widget> {
    let new_widget: gtk::Widget = match widget.name.as_str() {
        "slider" => {
            let slider_value: f64 = state.resolve(widget.attrs.get("value")?)?.as_f64()?;
            let slider_min: Option<f64> =
                try { state.resolve(widget.attrs.get("min")?)?.as_f64()? };
            let slider_min = slider_min.unwrap_or(0f64);
            let slider_max: Option<f64> =
                try { state.resolve(widget.attrs.get("max")?)?.as_f64()? };
            let slider_max = slider_max.unwrap_or(100f64);

            let on_change: Option<String> = try {
                state
                    .resolve(widget.attrs.get("onchange")?)?
                    .as_string()?
                    .clone()
            };

            let scale = gtk::Scale::new(
                gtk::Orientation::Horizontal,
                Some(&gtk::Adjustment::new(
                    slider_value,
                    slider_min,
                    slider_max,
                    1.0,
                    1.0,
                    1.0,
                )),
            );
            scale.set_property("draw-value", &false.to_value()).ok()?;

            if let Some(on_change) = on_change {
                scale.connect_value_changed(move |scale| {
                    run_command(&on_change, scale.get_value());
                });
            }

            scale.upcast()
        }

        name if widget_definitions.contains_key(name) => {
            let def = &widget_definitions[name];
            element_to_gtk_widget(widget_definitions, state, &def.structure)?
        }
        _ => return None,
    };
    Some(new_widget)
}

struct WidgetState(HashMap<String, config::AttrValue>);

impl WidgetState {
    pub fn resolve(&self, value: &config::AttrValue) -> Option<config::AttrValue> {
        if let config::AttrValue::VarRef(name) = value {
            // TODO REEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE
            self.0.get(name).cloned()
        } else {
            Some(value.clone())
        }
    }
}

fn run_command<T: std::fmt::Display>(cmd: &str, arg: T) {
    let cmd = cmd.replace(CMD_STRING_PLACEHODLER, &format!("{}", arg));
    if let Err(e) = Command::new("bash").arg("-c").arg(cmd).output() {
        eprintln!("{}", e);
    }
}

// macro_rules! build {
//     ($var_name:ident = $value:expr ; $code:block) => {{
//         let mut $var_name = $value;
//         $code;
//         $var_name
//     }};
// }
