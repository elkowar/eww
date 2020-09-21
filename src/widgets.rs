use crate::config::element;
use crate::eww_state::*;
use crate::value::{AttrValue, PrimitiveValue};
use anyhow::*;
use gtk::prelude::*;
use gtk::ImageExt;
use std::path::Path;
use std::{collections::HashMap, process::Command};

const CMD_STRING_PLACEHODLER: &str = "{}";

macro_rules! log_errors {
    ($body:expr) => {{
        let result = try { $body };
        if let Err(e) = result {
            eprintln!("WARN: {}", e);
        }
    }};
}

macro_rules! resolve {
    ($args:ident, $gtk_widget:ident, {
        $(
            $func:ident => $attr:literal $( = $default:literal)? $( = req $(@$required:tt)?)? => |$arg:ident| $body:expr
        ),+ $(,)?
    }) => {
        $(
            resolve!($args, $gtk_widget, $func => $attr $( [ $default ] )* $($($required)*)* => |$arg| $body);
        )+
    };

    ($args:ident, $gtk_widget:ident, {
        $($func:ident => {
            $($attr:literal $(= $default:literal)? $(= req $(@$required:tt)?)? => |$arg:ident| $body:expr),+ $(,)?
        }),+ $(,)?
    }) => {
        $($(
            resolve!($args, $gtk_widget, $func => $attr $( [ $default ] )* $($($required)*)* => |$arg| $body);
        )+)+
    };

    // optional
    ($args:ident, $gtk_widget:ident, $func:ident => $attr:literal => |$arg:ident| $body:expr) => {
        if let Some(attr_value) = $args.widget.attrs.get($attr) {
            $args.eww_state.$func($args.local_env, attr_value, {
                let $gtk_widget = $gtk_widget.clone();
                move |$arg| { $body; }
            });
        }
    };

    // required
    ($args:ident, $gtk_widget:ident, $func:ident => $attr:literal req => |$arg:ident| $body:expr) => {
        $args.eww_state.$func($args.local_env, $args.widget.get_attr($attr)?, {
            let $gtk_widget = $gtk_widget.clone();
            move |$arg| { $body; }
        });
    };

    // with default
    ($args:ident, $gtk_widget:ident, $func:ident => $attr:literal [$default:expr] => |$arg:ident| $body:expr) => {
        $args.eww_state.$func($args.local_env, $args.widget.attrs.get($attr).unwrap_or(&AttrValue::Concrete(PrimitiveValue::from($default))), {
            let $gtk_widget = $gtk_widget.clone();
            move |$arg| { $body; }
        });
    };
}

fn run_command<T: std::fmt::Display>(cmd: &str, arg: T) {
    let cmd = cmd.replace(CMD_STRING_PLACEHODLER, &format!("{}", arg));
    if let Err(e) = Command::new("bash").arg("-c").arg(cmd).output() {
        eprintln!("{}", e);
    }
}

pub fn element_to_gtk_thing(
    widget_definitions: &HashMap<String, element::WidgetDefinition>,
    eww_state: &mut EwwState,
    local_env: &HashMap<String, AttrValue>,
    element: &element::ElementUse,
) -> Result<gtk::Widget> {
    match element {
        element::ElementUse::Text(text) => Ok(gtk::Label::new(Some(&text)).upcast()),
        element::ElementUse::Widget(widget) => {
            let gtk_container =
                build_gtk_widget_or_container(widget_definitions, eww_state, local_env, widget)?;

            let gtk_widget = if let Some(gtk_container) = gtk_container {
                gtk_container
            } else if let Some(def) = widget_definitions.get(widget.name.as_str()) {
                let mut local_env = local_env.clone();
                local_env.extend(widget.attrs.clone());
                element_to_gtk_thing(widget_definitions, eww_state, &local_env, &def.structure)?
            } else {
                return Err(anyhow!("unknown widget: '{}'", &widget.name));
            };

            if let Ok(css_class) = widget
                .get_attr("class")
                .and_then(|x| AttrValue::as_string(x))
            {
                gtk_widget.get_style_context().add_class(css_class);
            }

            Ok(gtk_widget)
        }
    }
}

struct BuilderArgs<'a, 'b, 'c> {
    eww_state: &'a mut EwwState,
    local_env: &'b HashMap<String, AttrValue>,
    widget: &'c element::WidgetUse,
}

pub fn build_gtk_widget_or_container(
    widget_definitions: &HashMap<String, element::WidgetDefinition>,
    eww_state: &mut EwwState,
    local_env: &HashMap<String, AttrValue>,
    widget: &element::WidgetUse,
) -> Result<Option<gtk::Widget>> {
    let mut builder_args = BuilderArgs {
        eww_state,
        local_env: &local_env,
        widget: &widget,
    };
    if let Some(gtk_widget) = build_gtk_container(&mut builder_args)? {
        for child in &widget.children {
            let child_widget =
                &element_to_gtk_thing(widget_definitions, eww_state, local_env, child)
                    .with_context(|| {
                        format!(
                            "error while building child '{:?}' of '{}'",
                            &child,
                            &gtk_widget.get_widget_name()
                        )
                    })?;
            gtk_widget.add(child_widget);
        }
        Ok(Some(gtk_widget.upcast()))
    } else {
        build_gtk_widget(&mut builder_args).context("error building gtk widget")
    }
}

// widget definitions

fn build_gtk_widget(builder_args: &mut BuilderArgs) -> Result<Option<gtk::Widget>> {
    let gtk_widget = match builder_args.widget.name.as_str() {
        "slider" => build_gtk_scale(builder_args)?.upcast(),
        "image" => build_gtk_image(builder_args)?.upcast(),
        _ => return Ok(None),
    };
    Ok(Some(gtk_widget))
}

fn build_gtk_container(builder_args: &mut BuilderArgs) -> Result<Option<gtk::Container>> {
    let gtk_widget = match builder_args.widget.name.as_str() {
        "layout" => build_gtk_layout(builder_args)?.upcast(),
        "button" => build_gtk_button(builder_args)?.upcast(),
        _ => return Ok(None),
    };
    Ok(Some(gtk_widget))
}

// concrete widgets

fn build_gtk_scale(builder_args: &mut BuilderArgs) -> Result<gtk::Scale> {
    let gtk_widget = gtk::Scale::new(
        gtk::Orientation::Horizontal,
        Some(&gtk::Adjustment::new(0.0, 0.0, 100.0, 1.0, 1.0, 1.0)),
    );

    resolve!(builder_args, gtk_widget, {
        resolve_f64 => {
            "value" = req => |v| gtk_widget.set_value(v),
            "min"         => |v| gtk_widget.get_adjustment().set_lower(v),
            "max"         => |v| gtk_widget.get_adjustment().set_upper(v),
        },
        resolve_str => {
            "orientation" => |v| gtk_widget.set_orientation(parse_orientation(&v)),
            "onchange" => |cmd| {
                gtk_widget.connect_value_changed(move |gtk_widget| {
                    run_command(&cmd, gtk_widget.get_value());
                });
            }
        }
    });
    Ok(gtk_widget)
}

fn build_gtk_button(builder_args: &mut BuilderArgs) -> Result<gtk::Button> {
    let gtk_widget = gtk::Button::new();
    resolve!(builder_args, gtk_widget, {
        resolve_bool => "active" = true => |v| gtk_widget.set_sensitive(v),
        resolve_str  => "onclick"       => |v| gtk_widget.connect_clicked(move |_| run_command(&v, ""))

    });
    Ok(gtk_widget)
}

fn build_gtk_image(builder_args: &mut BuilderArgs) -> Result<gtk::Image> {
    let gtk_widget = gtk::Image::new();
    resolve!(builder_args, gtk_widget, {
        resolve_str => "path" = req => |v| gtk_widget.set_from_file(Path::new(&v))
    });
    Ok(gtk_widget)
}

fn build_gtk_layout(builder_args: &mut BuilderArgs) -> Result<gtk::Box> {
    let gtk_widget = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    resolve!(builder_args, gtk_widget, {
        resolve_f64 => "spacing" = 10.0 => |v| gtk_widget.set_spacing(v as i32),
        resolve_str => "orientation"    => |v| gtk_widget.set_orientation(parse_orientation(&v)),

    });
    Ok(gtk_widget)
}

fn parse_orientation(o: &str) -> gtk::Orientation {
    match o {
        "vertical" => gtk::Orientation::Vertical,
        _ => gtk::Orientation::Horizontal,
    }
}
