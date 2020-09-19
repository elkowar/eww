#![feature(try_blocks)]
extern crate gio;
extern crate gtk;

use anyhow::{self, Context, Result};
use gdk::*;
use gio::prelude::*;
use gtk::prelude::*;
use gtk::{Adjustment, Application, ApplicationWindow, Button, Scale};
use regex::Regex;
use std::{collections::HashMap, str::FromStr};

pub mod config;
pub mod widgets;

fn main() -> Result<()> {
    let application = Application::new(Some("de.elkowar.eww"), Default::default())
        .expect("failed to initialize GTK application");

    application.connect_activate(|app| {
        let window = ApplicationWindow::new(app);
        window.set_title("Eww");
        window.set_wmclass("noswallow", "noswallow");
        window.set_type_hint(gdk::WindowTypeHint::Dock);
        window.set_position(gtk::WindowPosition::Center);
        window.set_keep_above(true);

        let element = config::parse_element_use(
            config::parse_hocon(
                r#"{
            layout_horizontal: {
                children: [
                    "hi",
                    { button: { children: "click me you" } }
                    { slider: {} }
                    "hu"
                ]
            }
        }"#,
            )
            .unwrap(),
        )
        .unwrap();

        let widget_state = WidgetState(HashMap::new());

        window.add(&element_to_gtk_widget(&widget_state, &element).unwrap());

        window.show_all();
    });

    application.run(&[]);
    Ok(())
}

fn element_to_gtk_widget(
    widget_state: &WidgetState,
    element: &config::ElementUse,
) -> Option<gtk::Widget> {
    match element {
        config::ElementUse::Text(text) => Some(gtk::Label::new(Some(&text)).upcast()),
        config::ElementUse::Widget(widget) => widget_use_to_gtk_container(widget_state, &widget)
            .or(widget_use_to_gtk_widget(widget_state, &widget)),
    }
}

fn widget_use_to_gtk_container(
    widget_state: &WidgetState,
    widget: &config::WidgetUse,
) -> Option<gtk::Widget> {
    let container_widget: gtk::Container = match widget.name.as_str() {
        "layout_horizontal" => gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast(),
        "button" => gtk::Button::new().upcast(),
        _ => return None,
    };

    for child in &widget.children {
        container_widget.add(&element_to_gtk_widget(widget_state, child)?);
    }
    Some(container_widget.upcast())
}

fn widget_use_to_gtk_widget(
    widget_state: &WidgetState,
    widget: &config::WidgetUse,
) -> Option<gtk::Widget> {
    let new_widget: gtk::Widget = match widget.name.as_str() {
        "slider" => {
            let slider_value: f64 = widget_state.resolve(widget.attrs.get("value")?)?;

            gtk::Scale::new(
                gtk::Orientation::Horizontal,
                Some(&gtk::Adjustment::new(
                    slider_value,
                    0.0,
                    100.0,
                    1.0,
                    1.0,
                    1.0,
                )),
            )
            .upcast()
        }

        _ => return None,
    };
    Some(new_widget)
}

struct WidgetState(HashMap<String, config::AttrValue>);

impl WidgetState {
    pub fn resolve<T>(&self, value: &config::AttrValue) -> Option<String>
    where
        T: FromStr,
    {
        let var_pattern: Regex = Regex::new(r"\$\$\{(.*)\}").unwrap();
        let config::AttrValue(value) = value;

        let mut missing_var: Option<String> = None;
        var_pattern.replace_all(value, |caps: &regex::Captures| {
            self.lookup_full::<T>(&caps[1]).unwrap_or_else(|| {
                missing_var = Some(caps[1].to_string());
                "missing".to_string()
            })
        });

        // TODO REEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEe

        unimplemented!();
    }

    pub fn lookup_full<T>(&self, key: &str) -> Option<String>
    where
        T: FromStr,
    {
        self.resolve::<T>(self.0.get(key)?)
    }
}

// macro_rules! build {
//     ($var_name:ident = $value:expr ; $code:block) => {{
//         let mut $var_name = $value;
//         $code;
//         $var_name
//     }};
// }
