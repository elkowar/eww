
use gdk;
use gtk::prelude::*;
use yuck::config::{
    window_definition::{WindowDefinition, WindowStacking},
    window_geometry::AnchorAlignment,
};

pub fn initialize_window(window_def: &WindowDefinition, monitor: gdk::Rectangle) -> Option<gtk::Window> {
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    // Initialising a layer shell surface
    gtk_layer_shell::init_for_window(&window);
    // Sets the monitor where the surface is shown
    match window_def.monitor_number {
        Some(index) => {
            if let Some(monitor) = gdk::Display::default().expect("could not get default display").monitor(index) {
                gtk_layer_shell::set_monitor(&window, &monitor);
            } else {
                return None;
            }
        }
        None => {}
    };
    window.set_resizable(window_def.resizable);

    // Sets the layer where the layer shell surface will spawn
    match window_def.stacking {
        WindowStacking::Foreground => gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Top),
        WindowStacking::Background => gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Background),
        WindowStacking::Bottom => gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Bottom),
        WindowStacking::Overlay => gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Overlay),
    }

    // Sets the keyboard interactivity
    gtk_layer_shell::set_keyboard_interactivity(&window, window_def.backend_options.focusable);

    if let Some(geometry) = window_def.geometry {
        // Positioning surface
        let mut top = false;
        let mut left = false;
        let mut right = false;
        let mut bottom = false;

        match geometry.anchor_point.x {
            AnchorAlignment::START => left = true,
            AnchorAlignment::CENTER => {}
            AnchorAlignment::END => right = true,
        }
        match geometry.anchor_point.y {
            AnchorAlignment::START => top = true,
            AnchorAlignment::CENTER => {}
            AnchorAlignment::END => bottom = true,
        }

        gtk_layer_shell::set_anchor(&window, gtk_layer_shell::Edge::Left, left);
        gtk_layer_shell::set_anchor(&window, gtk_layer_shell::Edge::Right, right);
        gtk_layer_shell::set_anchor(&window, gtk_layer_shell::Edge::Top, top);
        gtk_layer_shell::set_anchor(&window, gtk_layer_shell::Edge::Bottom, bottom);

        let xoffset = geometry.offset.x.relative_to(monitor.width);
        let yoffset = geometry.offset.y.relative_to(monitor.height);

        if left {
            gtk_layer_shell::set_margin(&window, gtk_layer_shell::Edge::Left, xoffset);
        } else {
            gtk_layer_shell::set_margin(&window, gtk_layer_shell::Edge::Right, xoffset);
        }
        if bottom {
            gtk_layer_shell::set_margin(&window, gtk_layer_shell::Edge::Bottom, yoffset);
        } else {
            gtk_layer_shell::set_margin(&window, gtk_layer_shell::Edge::Top, yoffset);
        }
    }
    if window_def.backend_options.exclusive {
        gtk_layer_shell::auto_exclusive_zone_enable(&window);
    }
    Some(window)
}
