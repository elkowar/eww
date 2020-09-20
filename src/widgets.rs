use crate::build;
use crate::config::element;
use crate::eww_state::*;
use crate::value::AttrValue;
use anyhow::*;
use gtk::prelude::*;
use std::{collections::HashMap, process::Command};

const CMD_STRING_PLACEHODLER: &str = "{}";

macro_rules! resolve {
    ($args:ident, $gtk_widget:ident, {
         $($func:ident =>
           {
             $($attr:literal => |$arg:ident| $body:expr),+
           }
         ),+
    }) => {
      $(
        $(
          $args.eww_state.$func($args.local_env, $args.widget.get_attr($attr)?, {
            let $gtk_widget = $gtk_widget.clone();
            move |$arg| { $body; }
          });
        )+
      )+
    }
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
                build_gtk_widget_or_container(widget_definitions, eww_state, local_env, widget);
            let gtk_widget = gtk_container.or_else(|_| {
                if let Some(def) = widget_definitions.get(widget.name.as_str()) {
                    let local_environment = build!(env = local_env.clone(); {
                        env.extend(widget.attrs.clone());
                    });

                    element_to_gtk_thing(
                        widget_definitions,
                        eww_state,
                        &local_environment,
                        &def.structure,
                    )
                } else {
                    Err(anyhow!("unknown widget {}", &widget.name))
                }
            })?;

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
) -> Result<gtk::Widget> {
    let mut builder_args = BuilderArgs {
        eww_state,
        local_env: &local_env,
        widget: &widget,
    };
    let gtk_widget: Option<gtk::Widget> =
        if let Some(gtk_widget) = build_gtk_container(&mut builder_args)? {
            for child in &widget.children {
                let child_widget =
                    &element_to_gtk_thing(widget_definitions, eww_state, local_env, child)?;
                gtk_widget.add(child_widget);
            }
            Some(gtk_widget.upcast())
        } else {
            build_gtk_widget(&mut builder_args)?
        };
    gtk_widget.context(format!("unknown widget {:?}", widget))
}

// widget definitions

fn build_gtk_widget(builder_args: &mut BuilderArgs) -> Result<Option<gtk::Widget>> {
    let gtk_widget = match builder_args.widget.name.as_str() {
        "slider" => build_gtk_scale(builder_args)?.upcast(),
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
        "value" => |v| gtk_widget.set_value(v),
        "min"   => |v| gtk_widget.get_adjustment().set_lower(v),
        "max"   => |v| gtk_widget.get_adjustment().set_upper(v)
      },
      resolve_string => {
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
      resolve_bool => {
        "active" => |v| gtk_widget.set_sensitive(v)
      },
      resolve_string => {
        "onclick" => |cmd| gtk_widget.connect_clicked(move |_| run_command(&cmd, ""))
      }
    });
    Ok(gtk_widget)
}

fn build_gtk_layout(builder_args: &mut BuilderArgs) -> Result<gtk::Box> {
    let gtk_widget = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    resolve!(builder_args, gtk_widget, {
      resolve_f64 => {
        "spacing" => |v| gtk_widget.set_spacing(v as i32)
      }
    });

    Ok(gtk_widget)
}
