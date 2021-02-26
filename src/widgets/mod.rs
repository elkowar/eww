use crate::{config::window_definition::WindowName, eww_state::*, print_result_err, value::AttrName};
use anyhow::*;
use gtk::prelude::*;
use itertools::Itertools;

use std::process::Command;
use widget_definitions::*;

pub mod widget_definitions;
pub mod widget_node;

const CMD_STRING_PLACEHODLER: &str = "{}";

/// Run a command that was provided as an attribute. This command may use a
/// placeholder ('{}') which will be replaced by the value provided as [`arg`]
pub(self) fn run_command<T: std::fmt::Display>(cmd: &str, arg: T) {
    let cmd = cmd.replace(CMD_STRING_PLACEHODLER, &format!("{}", arg));
    let command_result = Command::new("/bin/sh")
        .arg("-c")
        .arg(&cmd)
        .spawn()
        .and_then(|mut child| child.wait());
    print_result_err!(format!("executing command {}", &cmd), command_result);
}

struct BuilderArgs<'a, 'b, 'c, 'd> {
    eww_state: &'a mut EwwState,
    widget: &'b widget_node::Generic,
    unhandled_attrs: Vec<&'c AttrName>,
    window_name: &'d WindowName,
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
    eww_state: &mut EwwState,
    window_name: &WindowName,
    widget: &widget_node::Generic,
) -> Result<Option<gtk::Widget>> {
    let mut bargs = BuilderArgs {
        eww_state,
        widget,
        window_name,
        unhandled_attrs: widget.attrs.keys().collect(),
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
            let child_widget = child.render(bargs.eww_state, window_name).with_context(|| {
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

    if let Some(w) = gtk_widget.dynamic_cast_ref() {
        resolve_range_attrs(&mut bargs, &w)
    }
    if let Some(w) = gtk_widget.dynamic_cast_ref() {
        resolve_orientable_attrs(&mut bargs, &w)
    };
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
