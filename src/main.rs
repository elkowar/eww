extern crate gio;
extern crate gtk;

use anyhow::*;
use gdk::*;
use gio::prelude::*;
use gtk::prelude::*;
use gtk::{Adjustment, Application, ApplicationWindow, Button, Scale};

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
        window.add(&element_to_gtk_widget(&element).unwrap());

        window.show_all();
    });

    application.run(&[]);
    Ok(())
}

fn element_to_gtk_widget(element: &config::ElementUse) -> Option<gtk::Widget> {
    match element {
        config::ElementUse::Text(text) => Some(gtk::Label::new(Some(&text)).upcast()),
        config::ElementUse::Widget(widget) => {
            widget_use_to_gtk_container(&widget).or(widget_use_to_gtk_widget(&widget))
        }
    }
}

fn widget_use_to_gtk_container(widget: &config::WidgetUse) -> Option<gtk::Widget> {
    let container_widget: gtk::Container = match widget.name.as_str() {
        "layout_horizontal" => gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast(),
        "button" => gtk::Button::new().upcast(),
        _ => return None,
    };

    for child in &widget.children {
        container_widget.add(&element_to_gtk_widget(child)?);
    }
    Some(container_widget.upcast())
}

fn widget_use_to_gtk_widget(widget: &config::WidgetUse) -> Option<gtk::Widget> {
    let new_widget: gtk::Widget = match widget.name.as_str() {
        "slider" => gtk::Scale::new(
            gtk::Orientation::Horizontal,
            Some(&gtk::Adjustment::new(50.0, 0.0, 100.0, 1.0, 1.0, 1.0)),
        )
        .upcast(),

        _ => return None,
    };
    Some(new_widget)
}
