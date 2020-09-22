#![feature(trace_macros)]
#![feature(try_blocks)]
extern crate gio;
extern crate gtk;

use anyhow::*;
use gdk::*;
use gio::prelude::*;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow};
use std::collections::HashMap;

pub mod config;
pub mod eww_state;
pub mod value;
pub mod widgets;

use eww_state::*;
use value::PrimitiveValue;

#[macro_export]
macro_rules! build {
    ($var_name:ident = $value:expr ; $code:block) => {{
        let mut $var_name = $value;
        $code;
        $var_name
    }};
}

const EXAMPLE_CONFIG: &str = r#"{
    widgets: {
        some_widget: {
            structure: {
                layout: {
                    class: "container",
                    children: [
                        { layout: {
                            orientation: "v"
                            children: [
                                "fancy button"
                                { button: { children: "reeee" } }
                            ]
                        } }
                        { layout: {
                            children: [
                                "hi",
                                { button: { children: "click me you" } }
                                { slider: { value: "$$some_value", min: 0, max: 100, onchange: "notify-send 'changed' {}" } }
                                { slider: { value: "$$some_value", orientation: "vertical" } }
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
        }
    },
    default_vars: {
        ree: 12
    }
    windows: {
        main_window: {
            pos.x: 200
            pos.y: 1550
            size.x: 500
            size.y: 50
            widget: {
                test: {
                    ooph: "$$ree"
                }
            }
        }
    },
}"#;

#[derive(Debug)]
enum MuhhMsg {
    UpdateValue(String, PrimitiveValue),
}

fn main() {
    if let Err(e) = try_main() {
        eprintln!("{:?}", e);
    }
}

fn try_main() -> Result<()> {
    let eww_config = config::EwwConfig::from_hocon(&config::parse_hocon(EXAMPLE_CONFIG)?)?;

    let application = Application::new(Some("de.elkowar.eww"), gio::ApplicationFlags::FLAGS_NONE)
        .expect("failed to initialize GTK application ");

    let window_def = eww_config.get_windows()["main_window"].clone();

    application.connect_activate(move |app| {
        let result: Result<()> = try {
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
                    .or_else(|| app_window.get_display().get_default_screen().get_system_visual())
                    .as_ref(),
            );

            app_window.fullscreen();

            let mut eww_state = EwwState::from_default_vars(eww_config.get_default_vars().clone());
            let empty_local_state = HashMap::new();

            app_window.add(&widgets::element_to_gtk_thing(
                &eww_config.get_widgets(),
                &mut eww_state,
                &empty_local_state,
                &window_def.widget,
            )?);

            app_window.show_all();

            let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
            std::thread::spawn(move || event_loop(tx));

            rx.attach(None, move |msg| {
                match msg {
                    MuhhMsg::UpdateValue(key, value) => eww_state.update_value(key, value),
                }

                glib::Continue(true)
            });

            let window = app_window.get_window().unwrap();
            window.set_override_redirect(true);
            window.move_(window_def.position.0, window_def.position.1);
            window.show();
            window.raise();
        };
        if let Err(err) = result {
            eprintln!("{:?}", err);
            std::process::exit(1);
        }
    });

    application.run(&[]);

    Ok(())
}

fn event_loop(sender: glib::Sender<MuhhMsg>) {
    let mut x = 0;
    loop {
        x += 1;
        std::thread::sleep(std::time::Duration::from_millis(1000));
        sender
            .send(MuhhMsg::UpdateValue(
                "ree".to_string(),
                PrimitiveValue::Number(x as f64 * 10.0),
            ))
            .unwrap();
    }
}
