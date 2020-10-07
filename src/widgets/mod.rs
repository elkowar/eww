use crate::config::element;
use crate::eww_state::*;
use crate::value::{AttrValue, VarName};
use anyhow::*;
use gtk::prelude::*;
use itertools::Itertools;
use ref_cast::RefCast;
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
    local_env: &'b HashMap<VarName, AttrValue>,
    widget: &'c element::WidgetUse,
    unhandled_attrs: Vec<&'c str>,
}

pub fn element_to_gtk_thing(
    widget_definitions: &HashMap<String, element::WidgetDefinition>,
    eww_state: &mut EwwState,
    local_env: &HashMap<VarName, AttrValue>,
    widget: &element::WidgetUse,
) -> Result<gtk::Widget> {
    let gtk_container = build_gtk_widget(widget_definitions, eww_state, local_env, widget)?;

    let gtk_widget = if let Some(gtk_container) = gtk_container {
        gtk_container
    } else if let Some(def) = widget_definitions.get(widget.name.as_str()) {
        // TODO widget cleanup phase, where widget arguments are resolved as far as possible beforehand?
        let mut local_env = local_env.clone();
        local_env.extend(widget.attrs.clone().into_iter().map(|(k, v)| (VarName(k), v)));
        let custom_widget = element_to_gtk_thing(widget_definitions, eww_state, &local_env, &def.structure)?;
        custom_widget.get_style_context().add_class(widget.name.as_str());
        custom_widget
    } else {
        bail!("unknown widget: '{}'", &widget.name);
    };

    Ok(gtk_widget)
}

pub fn build_gtk_widget(
    widget_definitions: &HashMap<String, element::WidgetDefinition>,
    eww_state: &mut EwwState,
    local_env: &HashMap<VarName, AttrValue>,
    widget: &element::WidgetUse,
) -> Result<Option<gtk::Widget>> {
    let mut bargs = BuilderArgs {
        eww_state,
        local_env,
        widget,
        unhandled_attrs: widget.attrs.keys().map(|x| x.as_ref()).collect(),
    };
    let gtk_widget = match widget_to_gtk_widget(&mut bargs) {
        Ok(Some(gtk_widget)) => gtk_widget,
        result => {
            return result.with_context(|| {
                anyhow!(
                    "{}Error building widget {}",
                    bargs.widget.text_pos.map(|x| format!("{} |", x)).unwrap_or_default(),
                    bargs.widget.name,
                )
            })
        }
    };

    if let Some(gtk_widget) = gtk_widget.dynamic_cast_ref::<gtk::Container>() {
        resolve_container_attrs(&mut bargs, &gtk_widget);
        for child in &widget.children {
            let child_widget = element_to_gtk_thing(widget_definitions, bargs.eww_state, local_env, child);
            let child_widget = child_widget.with_context(|| {
                format!(
                    "{}error while building child '{:#?}' of '{}'",
                    widget.text_pos.map(|x| format!("{} |", x)).unwrap_or_default(),
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

    if !bargs.unhandled_attrs.is_empty() {
        eprintln!(
            "{}WARN: Unknown attribute used in {}: {}",
            widget.text_pos.map(|x| format!("{} |", x)).unwrap_or_default(),
            widget.name,
            bargs.unhandled_attrs.join(", ")
        )
    }

    Ok(Some(gtk_widget))
}

#[macro_export]
macro_rules! resolve_block {
    ($args:ident, $gtk_widget:ident, {
        $(
            prop( $( $attr_name:ident : $typecast_func:ident $(= $default:expr)?),*) $code:block
        ),+ $(,)?
    }) => {
        $({
            $(
                $args.unhandled_attrs.retain(|a| a != &::std::stringify!($attr_name).replace('_', "-"));
            )*

            let attr_map: Result<_> = try {
                ::maplit::hashmap! {
                    $(
                        ::std::stringify!($attr_name).to_owned() => resolve_block!(@get_value $args, &::std::stringify!($attr_name).replace('_', "-"), $(= $default)?)
                    ),*
                }
            };
            if let Ok(attr_map) = attr_map {
                $args.eww_state.resolve(
                    $args.local_env,
                    attr_map,
                    ::glib::clone!(@strong $gtk_widget => move |attrs| {
                        $(
                            let $attr_name = attrs.get( ::std::stringify!($attr_name) ).context("something went terribly wrong....")?.$typecast_func()?;
                        )*
                        $code
                        Ok(())
                    })
                );
            }
        })+
    };

    (@get_value $args:ident, $name:expr, = $default:expr) => {
        $args.widget.get_attr($name).cloned().unwrap_or(AttrValue::Concrete(PrimitiveValue::from($default)))
    };

    (@get_value $args:ident, $name:expr,) => {
        $args.widget.get_attr($name)?.clone()
    }
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
