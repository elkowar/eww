use super::{run_command, BuilderArgs};
use crate::resolve;
use crate::value::{AttrValue, PrimitiveValue};
use anyhow::*;
use gtk::prelude::*;
use gtk::ImageExt;
use std::path::Path;

// TODO figure out how to
// https://developer.gnome.org/gtk3/stable/GtkFixed.html

// general attributes

/// attributes that apply to all widgets
pub(super) fn resolve_widget_attrs(bargs: &mut BuilderArgs, gtk_widget: &gtk::Widget) {
    resolve!(bargs, gtk_widget, {
        resolve_str  => "class"         => |v| gtk_widget.get_style_context().add_class(&v),
        resolve_bool => "active" = true => |v| gtk_widget.set_sensitive(v),
        resolve_str  => "valign" => |v| gtk_widget.set_valign(parse_align(&v)),
        resolve_str  => "halign" => |v| gtk_widget.set_halign(parse_align(&v)),
    });
}

/// attributes that apply to all container widgets
pub(super) fn resolve_container_attrs(bargs: &mut BuilderArgs, gtk_widget: &gtk::Container) {
    resolve!(bargs, gtk_widget, {
        resolve_bool => "vexpand" = true => |v| gtk_widget.set_vexpand(v),
        resolve_bool => "hexpand" = true => |v| gtk_widget.set_hexpand(v),
    });
}

pub(super) fn resolve_range_attrs(bargs: &mut BuilderArgs, gtk_widget: &gtk::Range) {
    resolve!(bargs, gtk_widget, {
        resolve_f64 => "value" = req => |v| gtk_widget.set_value(v),
        resolve_f64 => "min"         => |v| gtk_widget.get_adjustment().set_lower(v),
        resolve_f64 => "max"         => |v| gtk_widget.get_adjustment().set_upper(v),
        resolve_str => "orientation" => |v| gtk_widget.set_orientation(parse_orientation(&v)),
        resolve_str => "onchange"    => |cmd| {
            gtk_widget.connect_value_changed(move |gtk_widget| {
                run_command(&cmd, gtk_widget.get_value());
            });
        }
    });
}

pub(super) fn resolve_orientable_attrs(bargs: &mut BuilderArgs, gtk_widget: &gtk::Range) {
    resolve!(bargs, gtk_widget, {
        resolve_str => "orientation" => |v| gtk_widget.set_orientation(parse_orientation(&v)),
    });
}

// widget definitions

pub(super) fn widget_to_gtk_widget(bargs: &mut BuilderArgs) -> Result<Option<gtk::Widget>> {
    let gtk_widget = match bargs.widget.name.as_str() {
        "layout" => build_gtk_layout(bargs)?.upcast(),
        "slider" => build_gtk_scale(bargs)?.upcast(),
        "image" => build_gtk_image(bargs)?.upcast(),
        "button" => build_gtk_button(bargs)?.upcast(),
        "label" => build_gtk_label(bargs)?.upcast(),
        "text" => build_gtk_text(bargs)?.upcast(),
        _ => return Ok(None),
    };
    Ok(Some(gtk_widget))
}

// concrete widgets

fn build_gtk_scale(bargs: &mut BuilderArgs) -> Result<gtk::Scale> {
    let gtk_widget = gtk::Scale::new(
        gtk::Orientation::Horizontal,
        Some(&gtk::Adjustment::new(0.0, 0.0, 100.0, 1.0, 1.0, 1.0)),
    );
    resolve!(bargs, gtk_widget, {
        resolve_bool => "flipped"            => |v| gtk_widget.set_inverted(v),
        resolve_bool => "draw-value" = false => |v| gtk_widget.set_draw_value(v),
    });
    Ok(gtk_widget)
}

fn build_gtk_button(bargs: &mut BuilderArgs) -> Result<gtk::Button> {
    let gtk_widget = gtk::Button::new();
    resolve!(bargs, gtk_widget, {
        resolve_str => "onclick" => |v| gtk_widget.connect_clicked(move |_| run_command(&v, ""))

    });
    Ok(gtk_widget)
}

fn build_gtk_image(bargs: &mut BuilderArgs) -> Result<gtk::Image> {
    let gtk_widget = gtk::Image::new();
    resolve!(bargs, gtk_widget, {
        resolve_str => "path" = req => |v| gtk_widget.set_from_file(Path::new(&v))
    });
    Ok(gtk_widget)
}

fn build_gtk_layout(bargs: &mut BuilderArgs) -> Result<gtk::Box> {
    let gtk_widget = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    resolve!(bargs, gtk_widget, {
        resolve_f64  => "spacing" = 0.0     => |v| gtk_widget.set_spacing(v as i32),
        resolve_str  => "orientation"       => |v| gtk_widget.set_orientation(parse_orientation(&v)),
        resolve_bool => "homogenous" = true => |v| gtk_widget.set_homogeneous(v),
    });
    Ok(gtk_widget)
}

fn build_gtk_label(bargs: &mut BuilderArgs) -> Result<gtk::Label> {
    let gtk_widget = gtk::Label::new(None);
    resolve!(bargs, gtk_widget, {
        resolve_str => "text" => |v| gtk_widget.set_text(&v),
    });
    Ok(gtk_widget)
}

fn build_gtk_text(bargs: &mut BuilderArgs) -> Result<gtk::Label> {
    let text = bargs.widget.children.first().unwrap().get_attr("text")?;
    let gtk_widget = gtk::Label::new(None);
    bargs.eww_state.resolve_str(
        bargs.local_env,
        text,
        glib::clone!(@strong gtk_widget => move |v| gtk_widget.set_text(&v)),
    );
    Ok(gtk_widget)
}
fn parse_orientation(o: &str) -> gtk::Orientation {
    match o {
        "vertical" | "v" => gtk::Orientation::Vertical,
        _ => gtk::Orientation::Horizontal,
    }
}

fn parse_align(o: &str) -> gtk::Align {
    match o {
        "fill" => gtk::Align::Fill,
        "baseline" => gtk::Align::Baseline,
        "center" => gtk::Align::Center,
        "start" => gtk::Align::Start,
        "end" => gtk::Align::End,
        _ => gtk::Align::Start,
    }
}
