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

use config::element;
use eww_state::*;
use value::{AttrValue, PrimitiveValue};

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
                layout_horizontal: {
                    class: "container",
                    children: [
                        "hi",
                        { button: { children: "click me you" } }
                        { slider: { value: "$$some_value", min: 0, max: 100, onchange: "notify-send 'changed' {}" } }
                        { slider: { value: "$$some_value", min: 0, max: 100, onchange: "notify-send 'changed' {}" } }
                        "hu"
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
    dbg!(&eww_config);

    let application = Application::new(Some("de.elkowar.eww"), gio::ApplicationFlags::FLAGS_NONE)
        .expect("failed to initialize GTK application ");

    let window_def = eww_config.get_windows()["main_window"].clone();

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

        let mut eww_state = EwwState::from_default_vars(eww_config.get_default_vars().clone());
        let empty_local_state = HashMap::new();

        app_window.add(
            &element_to_gtk_thing(
                &eww_config.get_widgets(),
                &mut eww_state,
                &empty_local_state,
                &window_def.widget,
            )
            .unwrap(),
        );

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

fn element_to_gtk_thing(
    widget_definitions: &HashMap<String, element::WidgetDefinition>,
    eww_state: &mut EwwState,
    local_environment: &HashMap<String, AttrValue>,
    element: &element::ElementUse,
) -> Result<gtk::Widget> {
    match element {
        element::ElementUse::Text(text) => Ok(gtk::Label::new(Some(&text)).upcast()),
        element::ElementUse::Widget(widget) => {
            widget_use_to_gtk_thing(widget_definitions, eww_state, local_environment, widget)
        }
    }
}

fn widget_use_to_gtk_thing(
    widget_definitions: &HashMap<String, element::WidgetDefinition>,
    eww_state: &mut EwwState,
    local_environment: &HashMap<String, AttrValue>,
    widget: &element::WidgetUse,
) -> Result<gtk::Widget> {
    let gtk_widget =
        widget_use_to_gtk_container(widget_definitions, eww_state, &local_environment, &widget)
            .or(widget_use_to_gtk_widget(
                widget_definitions,
                eww_state,
                &local_environment,
                &widget,
            ))?;
    if let Some(css_class) = widget
        .attrs
        .get("class")
        .and_then(|x| AttrValue::as_string(x).ok())
    {
        gtk_widget.get_style_context().add_class(css_class);
    }

    Ok(gtk_widget)
}

fn widget_use_to_gtk_container(
    widget_definitions: &HashMap<String, element::WidgetDefinition>,
    eww_state: &mut EwwState,
    local_environment: &HashMap<String, AttrValue>,
    widget: &element::WidgetUse,
) -> Result<gtk::Widget> {
    let container_widget: gtk::Container = match widget.name.as_str() {
        "layout_horizontal" => gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast(),
        "button" => gtk::Button::new().upcast(),
        _ => return Err(anyhow!("{} is not a known container widget", widget.name)),
    };

    for child in &widget.children {
        container_widget.add(&element_to_gtk_thing(
            widget_definitions,
            eww_state,
            local_environment,
            child,
        )?);
    }
    Ok(container_widget.upcast())
}

fn widget_use_to_gtk_widget(
    widget_definitions: &HashMap<String, element::WidgetDefinition>,
    eww_state: &mut EwwState,
    local_env: &HashMap<String, AttrValue>,
    widget: &element::WidgetUse,
) -> Result<gtk::Widget> {
    let builder_args = widgets::BuilderArgs {
        eww_state,
        local_env: &local_env,
        widget: &widget,
    };
    let new_widget: gtk::Widget = match widget.name.as_str() {
        "slider" => widgets::build_gtk_scale(builder_args)?.upcast(),

        name if widget_definitions.contains_key(name) => {
            let def = &widget_definitions[name];
            let local_environment = build!(env = local_env.clone(); {
                env.extend(widget.attrs.clone());
            });

            element_to_gtk_thing(
                widget_definitions,
                eww_state,
                &local_environment,
                &def.structure,
            )?
        }
        _ => return Err(anyhow!("unknown widget {}", &widget.name)),
    };
    Ok(new_widget)
}
