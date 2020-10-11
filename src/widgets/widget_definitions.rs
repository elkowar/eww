use super::{run_command, BuilderArgs};
use crate::config;
use crate::eww_state;
use crate::resolve_block;
use crate::value::{AttrValue, PrimitiveValue};
use anyhow::*;
use gtk::prelude::*;
use gtk::ImageExt;
use maplit::hashmap;
use std::path::Path;

// TODO figure out how to
// TODO https://developer.gnome.org/gtk3/stable/GtkFixed.html

//// widget definitions

pub(super) fn widget_to_gtk_widget(bargs: &mut BuilderArgs) -> Result<Option<gtk::Widget>> {
    let gtk_widget = match bargs.widget.name.as_str() {
        "layout" => build_gtk_layout(bargs)?.upcast(),
        "slider" => build_gtk_scale(bargs)?.upcast(),
        "image" => build_gtk_image(bargs)?.upcast(),
        "button" => build_gtk_button(bargs)?.upcast(),
        "label" => build_gtk_label(bargs)?.upcast(),
        "text" => build_gtk_text(bargs)?.upcast(),
        "aspect" => build_gtk_aspect_frame(bargs)?.upcast(),
        "literal" => build_gtk_literal(bargs)?.upcast(),
        _ => return Ok(None),
    };
    Ok(Some(gtk_widget))
}

/// attributes that apply to all widgets
pub(super) fn resolve_widget_attrs(bargs: &mut BuilderArgs, gtk_widget: &gtk::Widget) {
    resolve_block!(bargs, gtk_widget, {
        prop(class:   as_string) { gtk_widget.get_style_context().add_class(&class) },
        prop(valign:  as_string) { gtk_widget.set_valign(parse_align(&valign)?) },
        prop(halign:  as_string) { gtk_widget.set_halign(parse_align(&halign)?) },
        prop(width:   as_f64   ) { gtk_widget.set_size_request(width as i32, gtk_widget.get_allocated_height()) },
        prop(height:  as_f64   ) { gtk_widget.set_size_request(gtk_widget.get_allocated_width(), height as i32) },
        prop(active:  as_bool = true) { gtk_widget.set_sensitive(active) },
        prop(visible: as_bool  ) {
            // TODO how do i call this only after the widget has been mapped? this is actually an issue,....
            if visible { gtk_widget.show(); } else { gtk_widget.hide(); }
        },
    });
}

/// attributes that apply to all container widgets
pub(super) fn resolve_container_attrs(bargs: &mut BuilderArgs, gtk_widget: &gtk::Container) {
    resolve_block!(bargs, gtk_widget, {
        prop(vexpand: as_bool = false) { gtk_widget.set_vexpand(vexpand) },
        prop(hexpand: as_bool = false) { gtk_widget.set_hexpand(hexpand) },
    });
}

pub(super) fn resolve_range_attrs(bargs: &mut BuilderArgs, gtk_widget: &gtk::Range) {
    resolve_block!(bargs, gtk_widget, {
        prop(value       : as_f64)    { gtk_widget.set_value(value)},
        prop(min         : as_f64)    { gtk_widget.get_adjustment().set_lower(min)},
        prop(max         : as_f64)    { gtk_widget.get_adjustment().set_upper(max)},
        prop(orientation : as_string) { gtk_widget.set_orientation(parse_orientation(&orientation)?) },
        prop(onchange    : as_string) {
            gtk_widget.connect_value_changed(move |gtk_widget| {
                run_command(&onchange, gtk_widget.get_value());
            });
        }
    });
}

pub(super) fn resolve_orientable_attrs(bargs: &mut BuilderArgs, gtk_widget: &gtk::Range) {
    resolve_block!(bargs, gtk_widget, {
        prop(orientation: as_string) { gtk_widget.set_orientation(parse_orientation(&orientation)?) },
    });
}

// concrete widgets

fn build_gtk_scale(bargs: &mut BuilderArgs) -> Result<gtk::Scale> {
    let gtk_widget = gtk::Scale::new(
        gtk::Orientation::Horizontal,
        Some(&gtk::Adjustment::new(0.0, 0.0, 100.0, 1.0, 1.0, 1.0)),
    );
    resolve_block!(bargs, gtk_widget, {
        prop(flipped: as_bool)            { gtk_widget.set_inverted(flipped) },
        prop(draw_value: as_bool = false) { gtk_widget.set_draw_value(draw_value) },
    });
    Ok(gtk_widget)
}

fn build_gtk_button(bargs: &mut BuilderArgs) -> Result<gtk::Button> {
    let gtk_widget = gtk::Button::new();
    resolve_block!(bargs, gtk_widget, {
        prop(onclick: as_string) { gtk_widget.connect_clicked(move |_| run_command(&onclick, "")); }
    });
    Ok(gtk_widget)
}

fn build_gtk_image(bargs: &mut BuilderArgs) -> Result<gtk::Image> {
    let gtk_widget = gtk::Image::new();
    resolve_block!(bargs, gtk_widget, {
        prop(path: as_string) { gtk_widget.set_from_file(Path::new(&path)); }
    });
    Ok(gtk_widget)
}

fn build_gtk_layout(bargs: &mut BuilderArgs) -> Result<gtk::Box> {
    let gtk_widget = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    resolve_block!(bargs, gtk_widget, {
        prop(spacing: as_f64  = 0.0)       { gtk_widget.set_spacing(spacing as i32) },
        prop(orientation: as_string)       { gtk_widget.set_orientation(parse_orientation(&orientation)?) },
        prop(space_evenly: as_bool = true) { gtk_widget.set_homogeneous(space_evenly) },
    });
    Ok(gtk_widget)
}

fn build_gtk_label(bargs: &mut BuilderArgs) -> Result<gtk::Label> {
    let gtk_widget = gtk::Label::new(None);
    resolve_block!(bargs, gtk_widget, {
        prop(text: as_string) { gtk_widget.set_text(&text) },
    });
    Ok(gtk_widget)
}

// TODO this is rather ugly,.....
fn build_gtk_text(bargs: &mut BuilderArgs) -> Result<gtk::Label> {
    let text = bargs
        .widget
        .children
        .first()
        .context("text node must contain exactly one child")?
        .get_attr("text")?;
    let gtk_widget = gtk::Label::new(None);
    bargs.eww_state.resolve(
        bargs.window_name,
        bargs.local_env,
        hashmap! {"text".to_owned() => text.clone() },
        glib::clone!(@strong gtk_widget => move |v| { gtk_widget.set_text(&v.get("text").unwrap().as_string().unwrap()); Ok(())}),
    );
    Ok(gtk_widget)
}

fn build_gtk_literal(bargs: &mut BuilderArgs) -> Result<gtk::Frame> {
    let gtk_widget = gtk::Frame::new(None);
    // TODO these clones here are dumdum
    let window_name = bargs.window_name.clone();
    let widget_definitions = bargs.widget_definitions.clone();
    resolve_block!(bargs, gtk_widget, {
        prop(content: as_string) {
            gtk_widget.get_children().iter().for_each(|w| gtk_widget.remove(w));
            if !content.is_empty() {
                let document = roxmltree::Document::parse(&content)?;
                let content_widget_use = config::element::WidgetUse::from_xml_node(document.root_element().into())?;
                let child_widget = super::widget_use_to_gtk_widget(
                    &widget_definitions,
                    &mut eww_state::EwwState::default(),
                    &window_name,
                    &std::collections::HashMap::new(),
                    &content_widget_use,
                )?;
                gtk_widget.add(&child_widget);
                child_widget.show();
            }
        }
    });
    Ok(gtk_widget)
}

fn build_gtk_aspect_frame(_bargs: &mut BuilderArgs) -> Result<gtk::AspectFrame> {
    let gtk_widget = gtk::AspectFrame::new(None, 0.5, 0.5, 1.0, true);
    Ok(gtk_widget)
}

fn parse_orientation(o: &str) -> Result<gtk::Orientation> {
    Ok(match o {
        "vertical" | "v" => gtk::Orientation::Vertical,
        "horizontal" | "h" => gtk::Orientation::Horizontal,
        _ => bail!("Couldn't parse orientation: '{}'", o),
    })
}

fn parse_align(o: &str) -> Result<gtk::Align> {
    Ok(match o {
        "fill" => gtk::Align::Fill,
        "baseline" => gtk::Align::Baseline,
        "center" => gtk::Align::Center,
        "start" => gtk::Align::Start,
        "end" => gtk::Align::End,
        _ => bail!("Couldn't parse alignment: '{}'", o),
    })
}
