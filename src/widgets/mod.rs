use crate::config::element;
use crate::eww_state::*;
use crate::value::AttrValue;
use anyhow::*;
use gtk::prelude::*;
use std::{collections::HashMap, process::Command};
use widget_definitions::*;

pub mod widget_definitions;

const CMD_STRING_PLACEHODLER: &str = "{}";

pub fn run_command<T: std::fmt::Display>(cmd: &str, arg: T) {
    let cmd = cmd.replace(CMD_STRING_PLACEHODLER, &format!("{}", arg));
    if let Err(e) = Command::new("bash").arg("-c").arg(cmd).output() {
        eprintln!("{}", e);
    }
}

struct BuilderArgs<'a, 'b, 'c> {
    eww_state: &'a mut EwwState,
    local_env: &'b HashMap<String, AttrValue>,
    widget: &'c element::WidgetUse,
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
            let gtk_container = build_gtk_widget(widget_definitions, eww_state, local_env, widget)?;

            let gtk_widget = if let Some(gtk_container) = gtk_container {
                gtk_container
            } else if let Some(def) = widget_definitions.get(widget.name.as_str()) {
                let mut local_env = local_env.clone();
                local_env.extend(widget.attrs.clone());
                element_to_gtk_thing(widget_definitions, eww_state, &local_env, &def.structure)?
            } else {
                return Err(anyhow!("unknown widget: '{}'", &widget.name));
            };

            Ok(gtk_widget)
        }
    }
}

pub fn build_gtk_widget(
    widget_definitions: &HashMap<String, element::WidgetDefinition>,
    eww_state: &mut EwwState,
    local_env: &HashMap<String, AttrValue>,
    widget: &element::WidgetUse,
) -> Result<Option<gtk::Widget>> {
    let mut bargs = BuilderArgs {
        eww_state,
        local_env,
        widget,
    };
    let gtk_widget = match widget_to_gtk_widget(&mut bargs) {
        Ok(Some(gtk_widget)) => gtk_widget,
        result => return result,
    };

    if let Some(gtk_widget) = gtk_widget.dynamic_cast_ref::<gtk::Container>() {
        resolve_container_attrs(&mut bargs, &gtk_widget);
        for child in &widget.children {
            let child_widget = element_to_gtk_thing(widget_definitions, bargs.eww_state, local_env, child);
            let child_widget = child_widget.with_context(|| {
                format!(
                    "error while building child '{:?}' of '{}'",
                    &child,
                    &gtk_widget.get_widget_name()
                )
            })?;
            gtk_widget.add(&child_widget);
        }
    }
    gtk_widget.dynamic_cast_ref().map(|w| resolve_range_attrs(&mut bargs, &w));
    gtk_widget
        .dynamic_cast_ref()
        .map(|w| resolve_orientable_attrs(&mut bargs, &w));
    resolve_widget_attrs(&mut bargs, &gtk_widget);

    Ok(Some(gtk_widget))
}

#[macro_export]
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

#[allow(unused)]
macro_rules! log_errors {
    ($body:expr) => {{
        let result = try { $body };
        if let Err(e) = result {
            eprintln!("WARN: {}", e);
        }
    }};
}
