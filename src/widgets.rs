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

pub struct BuilderArgs<'a, 'b, 'c> {
    pub eww_state: &'a mut EwwState,
    pub local_env: &'b HashMap<String, AttrValue>,
    pub widget: &'c element::WidgetUse,
}

pub fn build_gtk_scale(builder_args: BuilderArgs) -> Result<gtk::Scale> {
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

pub fn build_gtk_button(builder_args: BuilderArgs) -> Result<gtk::Button> {
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

pub fn build_gtk_layout(builder_args: BuilderArgs) -> Result<gtk::Box> {
    let gtk_widget = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    resolve!(builder_args, gtk_widget, {
      resolve_f64 => {
        "spacing" => |v| gtk_widget.set_spacing(v as i32)
      }
    });

    Ok(gtk_widget)
}

//"layout_horizontal" => gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast(),

fn run_command<T: std::fmt::Display>(cmd: &str, arg: T) {
    let cmd = cmd.replace(CMD_STRING_PLACEHODLER, &format!("{}", arg));
    if let Err(e) = Command::new("bash").arg("-c").arg(cmd).output() {
        eprintln!("{}", e);
    }
}
