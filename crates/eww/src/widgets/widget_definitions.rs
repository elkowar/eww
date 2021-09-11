#![allow(clippy::option_map_unit_fn)]
use super::{run_command, BuilderArgs};
use crate::{
    enum_parse, error::DiagError, error_handling_ctx, eww_state, resolve_block, util::list_difference, widgets::widget_node,
};
use anyhow::*;
use gdk::WindowExt;
use glib;
use gtk::{self, prelude::*, ImageExt};
use itertools::Itertools;
use std::{cell::RefCell, cmp::Ordering, collections::HashMap, rc::Rc, time::Duration};
use yuck::{
    config::validate::ValidationError,
    error::{AstError, AstResult, AstResultExt},
    gen_diagnostic,
    parser::from_ast::FromAst,
};

// TODO figure out how to
// TODO https://developer.gnome.org/gtk3/stable/GtkFixed.html

//// widget definitions

pub(super) fn widget_to_gtk_widget(bargs: &mut BuilderArgs) -> Result<gtk::Widget> {
    let gtk_widget = match bargs.widget.name.as_str() {
        "box" => build_gtk_box(bargs)?.upcast(),
        "centerbox" => build_center_box(bargs)?.upcast(),
        "scale" => build_gtk_scale(bargs)?.upcast(),
        "progress" => build_gtk_progress(bargs)?.upcast(),
        "image" => build_gtk_image(bargs)?.upcast(),
        "button" => build_gtk_button(bargs)?.upcast(),
        "label" => build_gtk_label(bargs)?.upcast(),
        "literal" => build_gtk_literal(bargs)?.upcast(),
        "input" => build_gtk_input(bargs)?.upcast(),
        "calendar" => build_gtk_calendar(bargs)?.upcast(),
        "color-button" => build_gtk_color_button(bargs)?.upcast(),
        "expander" => build_gtk_expander(bargs)?.upcast(),
        "color-chooser" => build_gtk_color_chooser(bargs)?.upcast(),
        "combo-box-text" => build_gtk_combo_box_text(bargs)?.upcast(),
        "checkbox" => build_gtk_checkbox(bargs)?.upcast(),
        "revealer" => build_gtk_revealer(bargs)?.upcast(),
        "if-else" => build_if_else(bargs)?.upcast(),
        _ => {
            return Err(AstError::ValidationError(ValidationError::UnknownWidget(
                bargs.widget.name_span,
                bargs.widget.name.to_string(),
            ))
            .into())
        }
    };
    Ok(gtk_widget)
}

/// attributes that apply to all widgets
/// @widget widget
/// @desc these properties apply to _all_ widgets, and can be used anywhere!
pub(super) fn resolve_widget_attrs(bargs: &mut BuilderArgs, gtk_widget: &gtk::Widget) {
    let css_provider = gtk::CssProvider::new();

    if let Ok(visible) =
        bargs.widget.get_attr("visible").and_then(|v| bargs.eww_state.resolve_once(v)?.as_bool().map_err(|e| anyhow!(e)))
    {
        connect_first_map(gtk_widget, move |w| {
            if visible {
                w.show();
            } else {
                w.hide();
            }
        })
    }

    let on_scroll_handler_id: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));
    let on_hover_handler_id: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));
    let cursor_hover_enter_handler_id: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));
    let cursor_hover_leave_handler_id: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));

    resolve_block!(bargs, gtk_widget, {
        // @prop class - css class name
        prop(class: as_string) {
            let old_classes = gtk_widget.get_style_context().list_classes();
            let old_classes = old_classes.iter().map(|x| x.as_str()).collect::<Vec<&str>>();
            let new_classes = class.split(' ').collect::<Vec<_>>();
            let (missing, new) = list_difference(&old_classes, &new_classes);
            for class in missing {
                gtk_widget.get_style_context().remove_class(class);
            }
            for class in new {
                gtk_widget.get_style_context().add_class(class);
            }
        },
        // @prop valign - how to align this vertically. possible values: $alignment
        prop(valign: as_string) { gtk_widget.set_valign(parse_align(&valign)?) },
        // @prop halign - how to align this horizontally. possible values: $alignment
        prop(halign: as_string) { gtk_widget.set_halign(parse_align(&halign)?) },
        // @prop vexpand - should this container expand vertically. Default: false.
        prop(vexpand: as_bool = false) { gtk_widget.set_vexpand(vexpand) },
        // @prop hexpand - should this widget expand horizontally. Default: false.
        prop(hexpand: as_bool = false) { gtk_widget.set_hexpand(hexpand) },
        // @prop width - width of this element. note that this can not restrict the size if the contents stretch it
        prop(width: as_f64) { gtk_widget.set_size_request(width as i32, gtk_widget.get_allocated_height()) },
        // @prop height - height of this element. note that this can not restrict the size if the contents stretch it
        prop(height: as_f64) { gtk_widget.set_size_request(gtk_widget.get_allocated_width(), height as i32) },
        // @prop active - If this widget can be interacted with
        prop(active: as_bool = true) { gtk_widget.set_sensitive(active) },
        // @prop tooltip - tooltip text (on hover)
        prop(tooltip: as_string) {
            gtk_widget.set_tooltip_text(Some(&tooltip));
        },
        // @prop visible - visibility of the widget
        prop(visible: as_bool = true) {
            // TODO how do i call this only after the widget has been mapped? this is actually an issue,....
            if visible { gtk_widget.show(); } else { gtk_widget.hide(); }
        },
        // @prop style - inline css style applied to the widget
        prop(style: as_string) {
            gtk_widget.reset_style();
            css_provider.load_from_data(format!("* {{ {} }}", style).as_bytes())?;
            gtk_widget.get_style_context().add_provider(&css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION)
        },
        // @prop timeout - timeout of the command
        // @prop onscroll - event to execute when the user scrolls with the mouse over the widget. The placeholder `{}` used in the command will be replaced with either `up` or `down`.
        prop(timeout: as_duration = Duration::from_millis(200), onscroll: as_string) {
            gtk_widget.add_events(gdk::EventMask::SCROLL_MASK);
            gtk_widget.add_events(gdk::EventMask::SMOOTH_SCROLL_MASK);
            let old_id = on_scroll_handler_id.replace(Some(
                gtk_widget.connect_scroll_event(move |_, evt| {
                    run_command(timeout, &onscroll, if evt.get_delta().1 < 0f64 { "up" } else { "down" });
                    gtk::Inhibit(false)
                })
            ));
            old_id.map(|id| gtk_widget.disconnect(id));
        },
        // @prop timeout - timeout of the command
        // @prop onhover - event to execute when the user hovers over the widget
        prop(timeout: as_duration = Duration::from_millis(200),onhover: as_string) {
            gtk_widget.add_events(gdk::EventMask::ENTER_NOTIFY_MASK);
            let old_id = on_hover_handler_id.replace(Some(
                gtk_widget.connect_enter_notify_event(move |_, evt| {
                    run_command(timeout, &onhover, format!("{} {}", evt.get_position().0, evt.get_position().1));
                    gtk::Inhibit(false)
                })
            ));
            old_id.map(|id| gtk_widget.disconnect(id));
        },

        // @prop cursor - Cursor to show while hovering (see [gtk3-cursors](https://developer.gnome.org/gdk3/stable/gdk3-Cursors.html) for possible names)
        prop(cursor: as_string) {
            gtk_widget.add_events(gdk::EventMask::ENTER_NOTIFY_MASK);
            cursor_hover_enter_handler_id.replace(Some(
                gtk_widget.connect_enter_notify_event(move |widget, _evt| {
                    let display = gdk::Display::get_default();
                    let gdk_window = widget.get_window();
                    if let (Some(display), Some(gdk_window)) = (display, gdk_window) {
                        gdk_window.set_cursor(gdk::Cursor::from_name(&display, &cursor).as_ref());
                    }
                    gtk::Inhibit(false)
                })
            )).map(|id| gtk_widget.disconnect(id));

            cursor_hover_leave_handler_id.replace(Some(
                gtk_widget.connect_leave_notify_event(move |widget, _evt| {
                    let gdk_window = widget.get_window();
                    if let Some(gdk_window) = gdk_window {
                        gdk_window.set_cursor(None);
                    }
                    gtk::Inhibit(false)
                })
            )).map(|id| gtk_widget.disconnect(id));
        },
    });
}

/// @widget !container
pub(super) fn resolve_container_attrs(_bargs: &mut BuilderArgs, _gtk_widget: &gtk::Container) {
    // resolve_block!(bargs, gtk_widget, {});
}

/// @widget !range
pub(super) fn resolve_range_attrs(bargs: &mut BuilderArgs, gtk_widget: &gtk::Range) {
    let on_change_handler_id: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));
    gtk_widget.set_sensitive(false);

    // only allow changing the value via the value property if the user isn't currently dragging
    let is_being_dragged = Rc::new(RefCell::new(false));
    gtk_widget.connect_button_press_event(glib::clone!(@strong is_being_dragged => move |_, _| {
        *is_being_dragged.borrow_mut() = true;
        gtk::Inhibit(false)
    }));
    gtk_widget.connect_button_release_event(glib::clone!(@strong is_being_dragged => move |_, _| {
        *is_being_dragged.borrow_mut() = false;
        gtk::Inhibit(false)
    }));

    resolve_block!(bargs, gtk_widget, {
        // @prop value - the value
        prop(value: as_f64) {
            if !*is_being_dragged.borrow() {
                gtk_widget.set_value(value)
            }
        },
        // @prop min - the minimum value
        prop(min: as_f64) { gtk_widget.get_adjustment().set_lower(min)},
        // @prop max - the maximum value
        prop(max: as_f64) { gtk_widget.get_adjustment().set_upper(max)},
        // @prop timeout - timeout of the command
        // @prop onchange - command executed once the value is changes. The placeholder `{}`, used in the command will be replaced by the new value.
        prop(timeout: as_duration = Duration::from_millis(200), onchange: as_string) {
            gtk_widget.set_sensitive(true);
            gtk_widget.add_events(gdk::EventMask::ENTER_NOTIFY_MASK);
            let old_id = on_change_handler_id.replace(Some(
                gtk_widget.connect_value_changed(move |gtk_widget| {
                    run_command(timeout, &onchange, gtk_widget.get_value());
                })
            ));
            old_id.map(|id| gtk_widget.disconnect(id));

        }
    });
}

/// @widget !orientable
pub(super) fn resolve_orientable_attrs(bargs: &mut BuilderArgs, gtk_widget: &gtk::Range) {
    resolve_block!(bargs, gtk_widget, {
        // @prop orientation - orientation of the widget. Possible values: $orientation
        prop(orientation: as_string) { gtk_widget.set_orientation(parse_orientation(&orientation)?) },
    });
}

// concrete widgets

fn build_if_else(bargs: &mut BuilderArgs) -> Result<gtk::Box> {
    if bargs.widget.children.len() != 2 {
        bail!("if-widget needs to have exactly two children, but had {}", bargs.widget.children.len());
    }
    let gtk_widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let (yes_widget, no_widget) = (bargs.widget.children[0].clone(), bargs.widget.children[1].clone());

    let yes_widget = yes_widget.render(bargs.eww_state, bargs.window_name, bargs.widget_definitions)?;
    let no_widget = no_widget.render(bargs.eww_state, bargs.window_name, bargs.widget_definitions)?;

    resolve_block!(bargs, gtk_widget, {
        prop(cond: as_bool) {
            gtk_widget.get_children().iter().for_each(|w| gtk_widget.remove(w));
            if cond {
                gtk_widget.add(&yes_widget)
            } else {
                gtk_widget.add(&no_widget)
            }
        }
    });
    Ok(gtk_widget)
}

/// @widget combo-box-text
/// @desc A combo box allowing the user to choose between several items.
fn build_gtk_combo_box_text(bargs: &mut BuilderArgs) -> Result<gtk::ComboBoxText> {
    let gtk_widget = gtk::ComboBoxText::new();
    let on_change_handler_id: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));
    resolve_block!(bargs, gtk_widget, {
        // @prop items - Items that should be displayed in the combo box
        prop(items: as_vec) {
            gtk_widget.remove_all();
            for i in items {
                gtk_widget.append_text(&i);
            }
        },
        // @prop timeout - timeout of the command
        // @prop onchange - runs the code when a item was selected, replacing {} with the item as a string
        prop(timeout: as_duration = Duration::from_millis(200), onchange: as_string) {
            let old_id = on_change_handler_id.replace(Some(
                gtk_widget.connect_changed(move |gtk_widget| {
                    run_command(timeout, &onchange, gtk_widget.get_active_text().unwrap_or_else(|| "".into()));
                })
            ));
            old_id.map(|id| gtk_widget.disconnect(id));
        },
    });
    Ok(gtk_widget)
}
/// @widget expander extends container
/// @desc A widget that can expand and collapse, showing/hiding it's children.
fn build_gtk_expander(bargs: &mut BuilderArgs) -> Result<gtk::Expander> {
    let gtk_widget = gtk::Expander::new(None);
    resolve_block!(bargs, gtk_widget, {
        // @prop name - name of the expander
        prop(name: as_string) {gtk_widget.set_label(Some(&name));},
        // @prop expanded - sets if the tree is expanded
        prop(expanded: as_bool) { gtk_widget.set_expanded(expanded); }
    });
    Ok(gtk_widget)
}

/// @widget revealer extends container
/// @desc A widget that can reveal a child with an animation.
fn build_gtk_revealer(bargs: &mut BuilderArgs) -> Result<gtk::Revealer> {
    let gtk_widget = gtk::Revealer::new();
    resolve_block!(bargs, gtk_widget, {
        // @prop transition - the name of the transition. Possible values: $transition
        prop(transition: as_string = "crossfade") { gtk_widget.set_transition_type(parse_transition(&transition)?); },
        // @prop reveal - sets if the child is revealed or not
        prop(reveal: as_bool) { gtk_widget.set_reveal_child(reveal); },
        // @prop duration - the duration of the reveal transition
        prop(duration: as_duration = Duration::from_millis(500)) { gtk_widget.set_transition_duration(duration.as_millis() as u32); },
    });
    Ok(gtk_widget)
}

/// @widget a checkbox
/// @desc A checkbox that can trigger events on checked / unchecked.
fn build_gtk_checkbox(bargs: &mut BuilderArgs) -> Result<gtk::CheckButton> {
    let gtk_widget = gtk::CheckButton::new();
    let on_change_handler_id: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));
    resolve_block!(bargs, gtk_widget, {
        // @prop timeout - timeout of the command
        // @prop onchecked - action (command) to be executed when checked by the user
        // @prop onunchecked - similar to onchecked but when the widget is unchecked
        prop(timeout: as_duration = Duration::from_millis(200), onchecked: as_string = "", onunchecked: as_string = "") {
            let old_id = on_change_handler_id.replace(Some(
                gtk_widget.connect_toggled(move |gtk_widget| {
                    run_command(timeout, if gtk_widget.get_active() { &onchecked } else { &onunchecked }, "");
                })
            ));
            old_id.map(|id| gtk_widget.disconnect(id));
       }
    });

    Ok(gtk_widget)
}

/// @widget color-button
/// @desc A button opening a color chooser window
fn build_gtk_color_button(bargs: &mut BuilderArgs) -> Result<gtk::ColorButton> {
    let gtk_widget = gtk::ColorButtonBuilder::new().build();
    let on_change_handler_id: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));
    resolve_block!(bargs, gtk_widget, {
        // @prop use-alpha - bool to whether or not use alpha
        prop(use_alpha: as_bool) {gtk_widget.set_use_alpha(use_alpha);},

        // @prop onchange - runs the code when the color was selected
        // @prop timeout - timeout of the command
        prop(timeout: as_duration = Duration::from_millis(200), onchange: as_string) {
            let old_id = on_change_handler_id.replace(Some(
                gtk_widget.connect_color_set(move |gtk_widget| {
                    run_command(timeout, &onchange, gtk_widget.get_rgba());
                })
            ));
            old_id.map(|id| gtk_widget.disconnect(id));
        }
    });

    Ok(gtk_widget)
}

/// @widget color-chooser
/// @desc A color chooser widget
fn build_gtk_color_chooser(bargs: &mut BuilderArgs) -> Result<gtk::ColorChooserWidget> {
    let gtk_widget = gtk::ColorChooserWidget::new();
    let on_change_handler_id: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));
    resolve_block!(bargs, gtk_widget, {
        // @prop use-alpha - bool to wether or not use alpha
        prop(use_alpha: as_bool) {gtk_widget.set_use_alpha(use_alpha);},

        // @prop onchange - runs the code when the color was selected
        // @prop timeout - timeout of the command
        prop(timeout: as_duration = Duration::from_millis(200), onchange: as_string) {
            let old_id = on_change_handler_id.replace(Some(
                gtk_widget.connect_color_activated(move |_a, color| {
                    run_command(timeout, &onchange, *color);
                })
            ));
            old_id.map(|id| gtk_widget.disconnect(id));
        }
    });

    Ok(gtk_widget)
}

/// @widget scale extends range
/// @desc A slider.
fn build_gtk_scale(bargs: &mut BuilderArgs) -> Result<gtk::Scale> {
    let gtk_widget = gtk::Scale::new(gtk::Orientation::Horizontal, Some(&gtk::Adjustment::new(0.0, 0.0, 100.0, 1.0, 1.0, 1.0)));
    resolve_block!(bargs, gtk_widget, {
        // @prop flipped - flip the direction
        prop(flipped: as_bool) { gtk_widget.set_inverted(flipped) },

        // @prop draw-value - draw the value of the property
        prop(draw_value: as_bool = false) { gtk_widget.set_draw_value(draw_value) },
    });
    Ok(gtk_widget)
}

/// @widget progress
/// @desc A progress bar
fn build_gtk_progress(bargs: &mut BuilderArgs) -> Result<gtk::ProgressBar> {
    let gtk_widget = gtk::ProgressBar::new();
    resolve_block!(bargs, gtk_widget, {
        // @prop flipped - flip the direction
        prop(flipped: as_bool) { gtk_widget.set_inverted(flipped) },

        // @prop value - value of the progress bar (between 0-100)
        prop(value: as_f64) { gtk_widget.set_fraction(value / 100f64) },

        // @prop orientation - orientation of the progress bar. possible values: $orientation
        prop(orientation: as_string) { gtk_widget.set_orientation(parse_orientation(&orientation)?) },
    });
    Ok(gtk_widget)
}

/// @widget input
/// @desc An input field. For this to be useful, set `focusable="true"` on the window.
fn build_gtk_input(bargs: &mut BuilderArgs) -> Result<gtk::Entry> {
    let gtk_widget = gtk::Entry::new();
    let on_change_handler_id: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));
    resolve_block!(bargs, gtk_widget, {
        // @prop value - the content of the text field
        prop(value: as_string) {
            gtk_widget.set_text(&value);
        },

        // @prop onchange - Command to run when the text changes. The placeholder `{}` will be replaced by the value
        // @prop timeout - timeout of the command
        prop(timeout: as_duration = Duration::from_millis(200), onchange: as_string) {
            let old_id = on_change_handler_id.replace(Some(
                gtk_widget.connect_changed(move |gtk_widget| {
                    run_command(timeout, &onchange, gtk_widget.get_text().to_string());
                })
            ));
            old_id.map(|id| gtk_widget.disconnect(id));
        }
    });
    Ok(gtk_widget)
}

/// @widget button extends container
/// @desc A button
fn build_gtk_button(bargs: &mut BuilderArgs) -> Result<gtk::Button> {
    let gtk_widget = gtk::Button::new();
    let on_click_handler_id: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));

    resolve_block!(bargs, gtk_widget, {
        // @prop onclick - a command that get's run when the button is clicked
        // @prop onmiddleclick - a command that get's run when the button is middleclicked
        // @prop onrightclick - a command that get's run when the button is rightclicked
        // @prop timeout - timeout of the command
        prop(
            timeout: as_duration = Duration::from_millis(200),
            onclick: as_string = "",
            onmiddleclick: as_string = "",
            onrightclick: as_string = ""
        ) {
            gtk_widget.add_events(gdk::EventMask::ENTER_NOTIFY_MASK);
            let old_id = on_click_handler_id.replace(Some(
                gtk_widget.connect_button_press_event(move |_, evt| {
                    match evt.get_button() {
                        1 => run_command(timeout, &onclick, ""),
                        2 => run_command(timeout, &onmiddleclick, ""),
                        3 => run_command(timeout, &onrightclick, ""),
                        _ => {},
                    }
                    gtk::Inhibit(false)
                })
            ));
            old_id.map(|id| gtk_widget.disconnect(id));
        }

    });
    Ok(gtk_widget)
}

/// @widget image
/// @desc A widget displaying an image
fn build_gtk_image(bargs: &mut BuilderArgs) -> Result<gtk::Image> {
    let gtk_widget = gtk::Image::new();
    resolve_block!(bargs, gtk_widget, {
        // @prop path - path to the image file
        // @prop width - width of the image
        // @prop height - height of the image
        prop(path: as_string, width: as_i32 = 10000, height: as_i32 = 10000) {
            if path.ends_with(".gif") {
                let pixbuf_animation = gdk_pixbuf::PixbufAnimation::from_file(std::path::PathBuf::from(path))?;
                gtk_widget.set_from_animation(&pixbuf_animation);
            } else {
                let pixbuf = gdk_pixbuf::Pixbuf::from_file_at_size(std::path::PathBuf::from(path), width, height)?;
                gtk_widget.set_from_pixbuf(Some(&pixbuf));
            }
        }
    });
    Ok(gtk_widget)
}

/// @widget box extends container
/// @desc the main layout container
fn build_gtk_box(bargs: &mut BuilderArgs) -> Result<gtk::Box> {
    let gtk_widget = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    resolve_block!(bargs, gtk_widget, {
        // @prop spacing - spacing between elements
        prop(spacing: as_i32 = 0) { gtk_widget.set_spacing(spacing) },
        // @prop orientation - orientation of the box. possible values: $orientation
        prop(orientation: as_string) { gtk_widget.set_orientation(parse_orientation(&orientation)?) },
        // @prop space-evenly - space the widgets evenly.
        prop(space_evenly: as_bool = true) { gtk_widget.set_homogeneous(space_evenly) },
    });
    Ok(gtk_widget)
}

/// @widget centerbox extends container
/// @desc a box that must contain exactly three children, which will be layed out at the start, center and end of the container.
fn build_center_box(bargs: &mut BuilderArgs) -> Result<gtk::Box> {
    let gtk_widget = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    resolve_block!(bargs, gtk_widget, {
        // @prop orientation - orientation of the centerbox. possible values: $orientation
        prop(orientation: as_string) { gtk_widget.set_orientation(parse_orientation(&orientation)?) },
    });

    match bargs.widget.children.len().cmp(&3) {
        Ordering::Less => {
            Err(DiagError::new(gen_diagnostic!("centerbox must contain exactly 3 elements", bargs.widget.span)).into())
        }
        Ordering::Greater => {
            let (_, additional_children) = bargs.widget.children.split_at(3);
            // we know that there is more than three children, so unwrapping on first and left here is fine.
            let first_span = additional_children.first().unwrap().span();
            let last_span = additional_children.last().unwrap().span();
            Err(DiagError::new(gen_diagnostic!(
                "centerbox must contain exactly 3 elements, but got more",
                first_span.to(last_span)
            ))
            .into())
        }
        Ordering::Equal => {
            let mut children = bargs
                .widget
                .children
                .iter()
                .map(|child| child.render(bargs.eww_state, bargs.window_name, bargs.widget_definitions));
            // we know that we have exactly three children here, so we can unwrap here.
            let (first, center, end) = children.next_tuple().unwrap();
            let (first, center, end) = (first?, center?, end?);
            gtk_widget.pack_start(&first, true, true, 0);
            gtk_widget.set_center_widget(Some(&center));
            gtk_widget.pack_end(&end, true, true, 0);
            first.show();
            center.show();
            end.show();
            Ok(gtk_widget)
        }
    }
}

/// @widget label
/// @desc A text widget giving you more control over how the text is displayed
fn build_gtk_label(bargs: &mut BuilderArgs) -> Result<gtk::Label> {
    let gtk_widget = gtk::Label::new(None);

    resolve_block!(bargs, gtk_widget, {
        // @prop text - the text to display
        // @prop limit-width - maximum count of characters to display
        // @prop show_truncated - show whether the text was truncated
        prop(text: as_string, limit_width: as_i32 = i32::MAX, show_truncated: as_bool = true) {
            let truncated = text.chars().count() > limit_width as usize;
            let mut text = text.chars().take(limit_width as usize).collect::<String>();

            if show_truncated && truncated {
                text.push_str("...");
            }

            let text = unescape::unescape(&text).context(format!("Failed to unescape label text {}", &text))?;
            let text = unindent::unindent(&text);
            gtk_widget.set_text(&text);
        },
        // @prop markup - Pango markup to display
        prop(markup: as_string) { gtk_widget.set_markup(&markup); },
        // @prop wrap - Wrap the text. This mainly makes sense if you set the width of this widget.
        prop(wrap: as_bool) { gtk_widget.set_line_wrap(wrap) },
        // @prop angle - the angle of rotation for the label (between 0 - 360)
        prop(angle: as_f64 = 0) { gtk_widget.set_angle(angle) }
    });
    Ok(gtk_widget)
}

/// @widget literal
/// @desc A widget that allows you to render arbitrary yuck.
fn build_gtk_literal(bargs: &mut BuilderArgs) -> Result<gtk::Box> {
    let gtk_widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
    gtk_widget.set_widget_name("literal");

    // TODO these clones here are dumdum
    let window_name = bargs.window_name.to_string();
    let widget_definitions = bargs.widget_definitions.clone();
    let literal_use_span = bargs.widget.span;

    // the file id the literal-content has been stored under, for error reporting.
    let literal_file_id: Rc<RefCell<Option<usize>>> = Rc::new(RefCell::new(None));

    resolve_block!(bargs, gtk_widget, {
        // @prop content - inline yuck that will be rendered as a widget.
        prop(content: as_string) {
            gtk_widget.get_children().iter().for_each(|w| gtk_widget.remove(w));
            if !content.is_empty() {
                let widget_node_result: AstResult<_> = try {
                    let ast = {
                        let mut yuck_files = error_handling_ctx::YUCK_FILES.write().unwrap();
                        let (span, asts) = yuck_files.load_str("<literal-content>".to_string(), content)?;
                        if let Some(file_id) = literal_file_id.replace(Some(span.2)) {
                            yuck_files.unload(file_id);
                        }
                        yuck::parser::require_single_toplevel(span, asts)?
                    };

                    let content_widget_use = yuck::config::widget_use::WidgetUse::from_ast(ast)?;
                    widget_node::generate_generic_widget_node(&widget_definitions, &HashMap::new(), content_widget_use)?
                };

                let widget_node = widget_node_result.context_label(literal_use_span, "Error in the literal used here")?;
                let child_widget = widget_node.render(&mut eww_state::EwwState::default(), &window_name, &widget_definitions)
                    .map_err(|e| AstError::ErrorContext {
                        label_span: literal_use_span,
                        context: "Error in the literal used here".to_string(),
                        main_err: Box::new(error_handling_ctx::anyhow_err_to_diagnostic(&e).unwrap_or_else(|| gen_diagnostic!(e)))
                    })?;
                gtk_widget.add(&child_widget);
                child_widget.show();
            }
        }
    });
    Ok(gtk_widget)
}

/// @widget calendar
/// @desc A widget that displays a calendar
fn build_gtk_calendar(bargs: &mut BuilderArgs) -> Result<gtk::Calendar> {
    let gtk_widget = gtk::Calendar::new();
    let on_click_handler_id: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));
    resolve_block!(bargs, gtk_widget, {
        // @prop day - the selected day
        prop(day: as_f64) { gtk_widget.set_property_day(day as i32) },
        // @prop month - the selected month
        prop(month: as_f64) { gtk_widget.set_property_day(month as i32) },
        // @prop year - the selected year
        prop(year: as_f64) { gtk_widget.set_property_day(year as i32) },
        // @prop show-details - show details
        prop(show_details: as_bool) { gtk_widget.set_property_show_details(show_details) },
        // @prop show-heading - show heading line
        prop(show_heading: as_bool) { gtk_widget.set_property_show_heading(show_heading) },
        // @prop show-day-names - show names of days
        prop(show_day_names: as_bool) { gtk_widget.set_property_show_day_names(show_day_names) },
        // @prop show-week-numbers - show week numbers
        prop(show_week_numbers: as_bool) { gtk_widget.set_property_show_week_numbers(show_week_numbers) },
        // @prop onclick - command to run when the user selects a date. The `{}` placeholder will be replaced by the selected date.
        // @prop timeout - timeout of the command
        prop(timeout: as_duration = Duration::from_millis(200), onclick: as_string) {
            let old_id = on_click_handler_id.replace(Some(
                gtk_widget.connect_day_selected(move |w| {
                    run_command(
                        timeout,
                        &onclick,
                        format!("{}.{}.{}", w.get_property_day(), w.get_property_month(), w.get_property_year())
                    )
                })
            ));
            old_id.map(|id| gtk_widget.disconnect(id));
        }

    });

    Ok(gtk_widget)
}

/// @var orientation - "vertical", "v", "horizontal", "h"
fn parse_orientation(o: &str) -> Result<gtk::Orientation> {
    enum_parse! { "orientation", o,
        "vertical" | "v" => gtk::Orientation::Vertical,
        "horizontal" | "h" => gtk::Orientation::Horizontal,
    }
}

/// @var transition - "slideright", "slideleft", "slideup", "slidedown", "crossfade", "none"
fn parse_transition(t: &str) -> Result<gtk::RevealerTransitionType> {
    enum_parse! { "transition", t,
        "slideright" => gtk::RevealerTransitionType::SlideRight,
        "slideleft" => gtk::RevealerTransitionType::SlideLeft,
        "slideup" => gtk::RevealerTransitionType::SlideUp,
        "slidedown" => gtk::RevealerTransitionType::SlideDown,
        "fade" | "crossfade" => gtk::RevealerTransitionType::Crossfade,
        "none" => gtk::RevealerTransitionType::None,
    }
}

/// @var alignment - "fill", "baseline", "center", "start", "end"
fn parse_align(o: &str) -> Result<gtk::Align> {
    enum_parse! { "alignment", o,
        "fill" => gtk::Align::Fill,
        "baseline" => gtk::Align::Baseline,
        "center" => gtk::Align::Center,
        "start" => gtk::Align::Start,
        "end" => gtk::Align::End,
    }
}

fn connect_first_map<W: IsA<gtk::Widget>, F: Fn(&W) + 'static>(widget: &W, func: F) {
    // TODO it would be better to actually remove the connect_map after first map,
    // but that would be highly annoying to implement...
    let is_first_map = std::rc::Rc::new(std::cell::RefCell::new(true));
    widget.connect_map(move |w| {
        if is_first_map.replace(false) {
            func(w);
        }
    });
}
