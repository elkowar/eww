use crate::{
    config::{element::WidgetDefinition, window_definition::WindowName},
    eww_state::*,
    value::AttrName,
};
use anyhow::*;
use gtk::prelude::*;
use itertools::Itertools;
use std::collections::HashMap;

use std::process::Command;
use widget_definitions::*;

pub mod widget_definitions;
pub mod widget_node;

const CMD_STRING_PLACEHODLER: &str = "{}";

/// Run a command that was provided as an attribute. This command may use a
/// placeholder ('{}') which will be replaced by the value provided as [`arg`]
pub(self) fn run_command<T: 'static + std::fmt::Display + Send + Sync>(cmd: &str, arg: T) {
    use wait_timeout::ChildExt;
    let cmd = cmd.to_string();
    std::thread::spawn(move || {
        let cmd = cmd.replace(CMD_STRING_PLACEHODLER, &format!("{}", arg));
        log::debug!("Running command from widget: {}", cmd);
        let child = Command::new("/bin/sh").arg("-c").arg(&cmd).spawn();
        match child {
            Ok(mut child) => match child.wait_timeout(std::time::Duration::from_millis(200)) {
                // child timed out
                Ok(None) => {
                    log::error!("WARNING: command {} timed out", &cmd);
                    let _ = child.kill();
                    let _ = child.wait();
                }
                Err(err) => log::error!("Failed to execute command {}: {}", cmd, err),
                Ok(Some(_)) => {}
            },
            Err(err) => log::error!("Failed to launch child process: {}", err),
        }
    });
}

struct BuilderArgs<'a, 'b, 'c, 'd, 'e> {
    eww_state: &'a mut EwwState,
    widget: &'b widget_node::Generic,
    unhandled_attrs: Vec<&'c AttrName>,
    window_name: &'d WindowName,
    widget_definitions: &'e HashMap<String, WidgetDefinition>,
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
    widget_definitions: &HashMap<String, WidgetDefinition>,
    widget: &widget_node::Generic,
) -> Result<Option<gtk::Widget>> {
    let mut bargs =
        BuilderArgs { eww_state, widget, window_name, unhandled_attrs: widget.attrs.keys().collect(), widget_definitions };
    let gtk_widget = match widget_to_gtk_widget(&mut bargs) {
        Ok(Some(gtk_widget)) => gtk_widget,
        result => {
            return result.with_context(|| {
                format!(
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
            let child_widget = child.render(bargs.eww_state, window_name, widget_definitions).with_context(|| {
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
        log::error!(
            "{}: Unknown attribute used in {}: {}",
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
        $args.widget.get_attr($name).cloned().unwrap_or(AttrVal::from_primitive($default))
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
            log::warn!("{}", e);
        }
    }};
}
