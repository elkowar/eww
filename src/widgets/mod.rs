use crate::{
    config::{element, WindowName},
    eww_state::*,
    value::{AttrName, AttrValue, VarName},
};
use anyhow::*;
use gtk::prelude::*;
use itertools::Itertools;

use std::{collections::HashMap, process::Command};
use widget_definitions::*;

pub mod widget_definitions;

const CMD_STRING_PLACEHODLER: &str = "{}";

/// Run a command that was provided as an attribute. This command may use a
/// placeholder ('{}') which will be replaced by the value provided as [`arg`]
pub fn run_command<T: std::fmt::Display>(cmd: &str, arg: T) {
    let cmd = cmd.replace(CMD_STRING_PLACEHODLER, &format!("{}", arg));
    if let Err(e) = Command::new("/bin/sh").arg("-c").arg(cmd).output() {
        eprintln!("{}", e);
    }
}

struct BuilderArgs<'a, 'b, 'c, 'd, 'e> {
    eww_state: &'a mut EwwState,
    local_env: &'b HashMap<VarName, AttrValue>,
    widget: &'c element::WidgetUse,
    unhandled_attrs: Vec<&'c AttrName>,
    window_name: &'d WindowName,
    widget_definitions: &'e HashMap<String, element::WidgetDefinition>,
}

/// Generate a [gtk::Widget] from a [element::WidgetUse].
/// The widget_use may be using a builtin widget, or a custom
/// [element::WidgetDefinition].
///
/// Also registers all the necessary state-change handlers in the eww_state.
///
/// This may return `Err` in case there was an actual error while parsing or
/// resolving the widget, Or `Ok(None)` if the widget_use just didn't match any
/// widget name.
pub fn widget_use_to_gtk_widget(
    widget_definitions: &HashMap<String, element::WidgetDefinition>,
    eww_state: &mut EwwState,
    window_name: &WindowName,
    local_env: &HashMap<VarName, AttrValue>,
    widget: &element::WidgetUse,
) -> Result<gtk::Widget> {
    let builtin_gtk_widget = build_builtin_gtk_widget(widget_definitions, eww_state, window_name, local_env, widget)?;

    let gtk_widget = if let Some(builtin_gtk_widget) = builtin_gtk_widget {
        builtin_gtk_widget
    } else if let Some(def) = widget_definitions.get(widget.name.as_str()) {
        // let mut local_env = local_env.clone();

        // the attributes that are set on the widget need to be resolved as far as
        // possible. If an attribute is a variable reference, it must either reference a
        // variable in the current local_env, or in the global state. As we are building
        // widgets from the outer most to the most nested, we can resolve attributes at
        // every step. This way, any definition that is affected by changes in the
        // eww_state will be directly linked to the eww_state's value. Example:
        // foo="{{in_eww_state}}"  => attr_in_child="{{foo}}"  =>
        // attr_in_nested_child="{{attr_in_child}}" will be resolved step by step. This
        // code will first resolve attr_in_child to directly be
        // attr_in_child={{in_eww_state}}. then, in the widget_use_to_gtk_widget call of
        // that child element, attr_in_nested_child will again be resolved to point to
        // the value of attr_in_child, and thus: attr_in_nested_child="{{in_eww_state}}"
        let resolved_widget_attr_env = widget
            .attrs
            .clone()
            .into_iter()
            .map(|(attr_name, attr_value)| (VarName(attr_name.0), attr_value.resolve_one_level(local_env)))
            .collect();

        let custom_widget = widget_use_to_gtk_widget(
            widget_definitions,
            eww_state,
            window_name,
            &resolved_widget_attr_env,
            &def.structure,
        )?;
        custom_widget.get_style_context().add_class(widget.name.as_str());
        custom_widget
    } else {
        bail!("unknown widget: '{}'", &widget.name);
    };

    Ok(gtk_widget)
}

/// build a [`gtk::Widget`] out of a [`element::WidgetUse`] that uses a
/// **builtin widget**. User defined widgets are handled by
/// [widget_use_to_gtk_widget].
///
/// Also registers all the necessary handlers in the `eww_state`.
///
/// This may return `Err` in case there was an actual error while parsing or
/// resolving the widget, Or `Ok(None)` if the widget_use just didn't match any
/// widget name.
fn build_builtin_gtk_widget(
    widget_definitions: &HashMap<String, element::WidgetDefinition>,
    eww_state: &mut EwwState,
    window_name: &WindowName,
    local_env: &HashMap<VarName, AttrValue>,
    widget: &element::WidgetUse,
) -> Result<Option<gtk::Widget>> {
    let mut bargs = BuilderArgs {
        eww_state,
        local_env,
        widget,
        window_name,
        unhandled_attrs: widget.attrs.keys().collect(),
        widget_definitions,
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

    // run resolve functions for superclasses such as range, orientable, and widget

    if let Some(gtk_widget) = gtk_widget.dynamic_cast_ref::<gtk::Container>() {
        resolve_container_attrs(&mut bargs, &gtk_widget);
        for child in &widget.children {
            let child_widget = widget_use_to_gtk_widget(widget_definitions, bargs.eww_state, window_name, local_env, child);
            let child_widget = child_widget.with_context(|| {
                format!(
                    "{}error while building child '{:#?}' of '{}'",
                    widget.text_pos.map(|x| format!("{} |", x)).unwrap_or_default(),
                    &child,
                    &gtk_widget.get_widget_name()
                )
            })?;
            gtk_widget.add(&child_widget);
            child_widget.show();
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
            widget.text_pos.map(|x| format!("{} | ", x)).unwrap_or_default(),
            widget.name,
            bargs.unhandled_attrs.iter().map(|x| x.to_string()).join(", ")
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
                $args.unhandled_attrs.retain(|a| &a.0 != &::std::stringify!($attr_name).replace('_', "-"));
            )*

            let attr_map: Result<_> = try {
                ::maplit::hashmap! {
                    $(
                        crate::value::AttrName(::std::stringify!($attr_name).to_owned()) =>
                            resolve_block!(@get_value $args, &::std::stringify!($attr_name).replace('_', "-"), $(= $default)?)
                    ),*
                }
            };
            if let Ok(attr_map) = attr_map {
                $args.eww_state.resolve(
                    $args.window_name,
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
        $args.widget.get_attr($name).cloned().unwrap_or(AttrValue::from_primitive($default))
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
