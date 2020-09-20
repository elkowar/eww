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
pub mod value;
pub mod widgets;

use config::element;
use config::AttrValue;
use value::PrimitiveValue;

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

macro_rules! build {
    ($var_name:ident = $value:expr ; $code:block) => {{
        let mut $var_name = $value;
        $code;
        $var_name
    }};
}

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
) -> Option<gtk::Widget> {
    match element {
        element::ElementUse::Text(text) => Some(gtk::Label::new(Some(&text)).upcast()),
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
) -> Option<gtk::Widget> {
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

    Some(gtk_widget)
}

fn widget_use_to_gtk_container(
    widget_definitions: &HashMap<String, element::WidgetDefinition>,
    eww_state: &mut EwwState,
    local_environment: &HashMap<String, AttrValue>,
    widget: &element::WidgetUse,
) -> Option<gtk::Widget> {
    let container_widget: gtk::Container = match widget.name.as_str() {
        "layout_horizontal" => gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast(),
        "button" => gtk::Button::new().upcast(),
        _ => return None,
    };

    for child in &widget.children {
        container_widget.add(&element_to_gtk_thing(
            widget_definitions,
            eww_state,
            local_environment,
            child,
        )?);
    }
    Some(container_widget.upcast())
}

fn widget_use_to_gtk_widget(
    widget_definitions: &HashMap<String, element::WidgetDefinition>,
    eww_state: &mut EwwState,
    local_env: &HashMap<String, AttrValue>,
    widget: &element::WidgetUse,
) -> Option<gtk::Widget> {
    let new_widget: gtk::Widget = match widget.name.as_str() {
        "slider" => {
            let scale = gtk::Scale::new(
                gtk::Orientation::Horizontal,
                Some(&gtk::Adjustment::new(0.0, 0.0, 100.0, 1.0, 1.0, 1.0)),
            );
            eww_state.resolve_f64(local_env, widget.attrs.get("value")?, {
                let scale = scale.clone();
                move |value| scale.set_value(value)
            });
            eww_state.resolve_f64(local_env, widget.attrs.get("min")?, {
                let scale = scale.clone();
                move |value| scale.get_adjustment().set_lower(value)
            });
            eww_state.resolve_f64(local_env, widget.attrs.get("max")?, {
                let scale = scale.clone();
                move |value| scale.get_adjustment().set_upper(value)
            });
            eww_state.resolve_string(local_env, widget.attrs.get("onchange")?, {
                let scale = scale.clone();
                move |on_change| {
                    scale.connect_value_changed(move |scale| {
                        run_command(&on_change, scale.get_value());
                    });
                }
            });

            //scale.set_property("draw-value", &false.to_value()).ok()?;
            scale.upcast()
        }

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
        _ => return None,
    };
    Some(new_widget)
}

#[derive(Default)]
struct EwwState {
    on_change_handlers: HashMap<String, Vec<Box<dyn Fn(PrimitiveValue) + 'static>>>,
    state: HashMap<String, PrimitiveValue>,
}

impl EwwState {
    pub fn from_default_vars(defaults: HashMap<String, PrimitiveValue>) -> Self {
        EwwState {
            state: defaults,
            ..EwwState::default()
        }
    }
    pub fn update_value(&mut self, key: String, value: PrimitiveValue) {
        if let Some(handlers) = self.on_change_handlers.get(&key) {
            for on_change in handlers {
                on_change(value.clone());
            }
        }
        self.state.insert(key, value);
    }

    pub fn resolve<F: Fn(PrimitiveValue) + 'static + Clone>(
        &mut self,
        local_env: &HashMap<String, AttrValue>,
        value: &AttrValue,
        set_value: F,
    ) -> bool {
        dbg!("resolve: ", value);
        match value {
            AttrValue::VarRef(name) => {
                if let Some(value) = self.state.get(name).cloned() {
                    self.on_change_handlers
                        .entry(name.to_string())
                        .or_insert_with(Vec::new)
                        .push(Box::new(set_value.clone()));
                    self.resolve(local_env, &value.into(), set_value)
                } else if let Some(value) = local_env.get(name).cloned() {
                    self.resolve(local_env, &value, set_value)
                } else {
                    false
                }
            }
            AttrValue::Concrete(value) => {
                set_value(value.clone());
                true
            }
        }
    }

    pub fn resolve_f64<F: Fn(f64) + 'static + Clone>(
        &mut self,
        local_env: &HashMap<String, AttrValue>,
        value: &AttrValue,
        set_value: F,
    ) -> bool {
        self.resolve(local_env, value, move |x| {
            if let Err(e) = x.as_f64().map(|v| set_value(v)) {
                eprintln!("error while resolving value: {}", e);
            };
        })
    }

    #[allow(dead_code)]
    pub fn resolve_bool<F: Fn(bool) + 'static + Clone>(
        &mut self,
        local_env: &HashMap<String, AttrValue>,
        value: &AttrValue,
        set_value: F,
    ) -> bool {
        self.resolve(local_env, value, move |x| {
            if let Err(e) = x.as_bool().map(|v| set_value(v)) {
                eprintln!("error while resolving value: {}", e);
            };
        })
    }
    pub fn resolve_string<F: Fn(String) + 'static + Clone>(
        &mut self,
        local_env: &HashMap<String, AttrValue>,
        value: &AttrValue,
        set_value: F,
    ) -> bool {
        self.resolve(local_env, value, move |x| {
            if let Err(e) = x.as_string().map(|v| set_value(v.clone())) {
                eprintln!("error while resolving value: {}", e);
            };
        })
    }
}

fn run_command<T: std::fmt::Display>(cmd: &str, arg: T) {
    let cmd = cmd.replace(CMD_STRING_PLACEHODLER, &format!("{}", arg));
    if let Err(e) = Command::new("bash").arg("-c").arg(cmd).output() {
        eprintln!("{}", e);
    }
}
