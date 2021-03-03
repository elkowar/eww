pub use platform::*;

#[cfg(feature = "no-x11-wayland")]
mod platform {
    use crate::config::{Side, StrutDefinition};
    use anyhow::*;
    pub fn reserve_space_for(window: &gtk::Window, monitor: gdk::Rectangle, strut_def: StrutDefinition) -> Result<()> {
        Err(anyhow!("Cannot reserve space on non X11 or and wayland backends"))
    }
}

#[cfg(feature = "wayland")]
mod platform {
    use crate::{
        app::get_monitor,
        config::{EwwWindowDefinition, Side, StrutDefinition, WindowStacking},
    };
    use anyhow::*;
    use gtk::prelude::*;

    pub fn reserve_space_for(window: &gtk::Window, monitor: gdk::Rectangle, surface: StrutDefinition) -> Result<()> {
        // Initializing the layer surface
        let backend = LayerShellBackend::new()?;
        backend.reserve_space_for(window, monitor, surface);
        Ok(())
    }

    pub fn initialize_window(window_def: &mut EwwWindowDefinition) -> gtk::Window {
        let mut window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_resizable(true);
        // Inititialising a layer shell surface
        gtk_layer_shell::init_for_window(&window);
        // Sets the monitor where the surface is shown
        match window_def.screen_number {
            Some(index) => {
                let monitor = get_monitor(index);
                gtk_layer_shell::set_monitor(&window, &monitor);
            }
            None => {}
        };

        // Sets the layer where the layer shell surface will spawn
        match window_def.stacking {
            WindowStacking::Foreground => gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Top),
            WindowStacking::Background => gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Background),
            WindowStacking::Bottom => gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Bottom),
            WindowStacking::Overlay => gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Overlay),
        }

        // Sets the keyboard interactivity
        gtk_layer_shell::set_keyboard_interactivity(&window, window_def.focusable);
        window
    }

    struct LayerShellBackend {}

    impl LayerShellBackend {
        fn new() -> Result<Self> {
            Ok(LayerShellBackend {})
        }

        fn reserve_space_for(&self, window: &gtk::Window, monitor_rect: gdk::Rectangle, surface: StrutDefinition) {
            // Initialising a layer shell surface
            gtk_layer_shell::init_for_window(window);
            // Making the surface occupied by widget exclusive
            if surface.exclusive {
                gtk_layer_shell::auto_exclusive_zone_enable(window);
            }

            let mut top = false;
            let mut left = false;
            let mut right = false;
            let mut bottom = false;

            match surface.side {
                Side::Top => top = true,
                Side::Left => left = true,
                Side::Right => right = true,
                Side::Bottom => bottom = true,
                Side::Center => {}
                Side::TopLeft => {
                    top = true;
                    left = true;
                }
                Side::TopRight => {
                    top = true;
                    right = true;
                }
                Side::BottomRight => {
                    bottom = true;
                    right = true;
                }
                Side::BottomLeft => {
                    left = true;
                    bottom = true;
                }
            }

            gtk_layer_shell::set_anchor(window, gtk_layer_shell::Edge::Left, left);
            gtk_layer_shell::set_anchor(window, gtk_layer_shell::Edge::Right, right);
            gtk_layer_shell::set_anchor(window, gtk_layer_shell::Edge::Top, top);
            gtk_layer_shell::set_anchor(window, gtk_layer_shell::Edge::Bottom, bottom);

            let xoffset = surface.coords.x.relative_to(monitor_rect.width);
            let yoffset = surface.coords.y.relative_to(monitor_rect.height);

            if left {
                gtk_layer_shell::set_margin(window, gtk_layer_shell::Edge::Left, xoffset);
            } else {
                gtk_layer_shell::set_margin(window, gtk_layer_shell::Edge::Right, xoffset);
            }
            if bottom {
                gtk_layer_shell::set_margin(window, gtk_layer_shell::Edge::Bottom, yoffset);
            } else {
                gtk_layer_shell::set_margin(window, gtk_layer_shell::Edge::Top, yoffset);
            }
        }
    }
}

#[cfg(feature = "x11")]
mod platform {
    use crate::config::{EwwWindowDefinition, Side, StrutDefinition, WindowStacking};
    use anyhow::*;
    use gdkx11;
    use gtk::{self, prelude::*};
    use x11rb::protocol::xproto::ConnectionExt;

    use x11rb::{
        self,
        connection::Connection,
        protocol::xproto::*,
        rust_connection::{DefaultStream, RustConnection},
    };

    pub fn initialize_window(window_def: &mut EwwWindowDefinition) -> gtk::Window {
        let window = if window_def.focusable {
            gtk::Window::new(gtk::WindowType::Toplevel)
        } else {
            gtk::Window::new(gtk::WindowType::Popup)
        };
        window.set_resizable(true);
        if !window_def.focusable {
            window.set_type_hint(gdk::WindowTypeHint::Dock);
        }
        window
    }

    pub fn reserve_space_for(window: &gtk::Window, monitor: gdk::Rectangle, strut_def: StrutDefinition) -> Result<()> {
        let backend = X11Backend::new()?;
        backend.reserve_space_for(window, monitor, strut_def)?;
        Ok(())
    }

    struct X11Backend {
        conn: RustConnection<DefaultStream>,
        root_window: u32,
        atoms: AtomCollection,
    }

    impl X11Backend {
        fn new() -> Result<Self> {
            let (conn, screen_num) = RustConnection::connect(None)?;
            let screen = conn.setup().roots[screen_num].clone();
            let atoms = AtomCollection::new(&conn)?.reply()?;
            Ok(X11Backend { conn, root_window: screen.root, atoms })
        }

        fn reserve_space_for(
            &self,
            window: &gtk::Window,
            monitor_rect: gdk::Rectangle,
            strut_def: StrutDefinition,
        ) -> Result<()> {
            let win_id = window
                .get_window()
                .context("Couldn't get gdk window from gtk window")?
                .downcast::<gdkx11::X11Window>()
                .ok()
                .context("Failed to get x11 window for gtk window")?
                .get_xid() as u32;
            let root_window_geometry = self.conn.get_geometry(self.root_window)?.reply()?;

            let mon_end_x = (monitor_rect.x + monitor_rect.width) as u32 - 1u32;
            let mon_end_y = (monitor_rect.y + monitor_rect.height) as u32 - 1u32;

            let dist = match strut_def.side {
                Side::Left | Side::Right => strut_def.dist.relative_to(monitor_rect.width) as u32,
                Side::Top | Side::Bottom => strut_def.dist.relative_to(monitor_rect.height) as u32,
                _ => (monitor_rect.height / 2) as u32,
            };

            match strut_def.stacking {
                WindowStacking::Foreground=> {
                    gdk_window.raise();
                    window.set_keep_above(true);
                }
                WindowStacking::Background=> {
                    gdk_window.lower();
                    window.set_keep_below(true);
                }
                _=>{},
            }

            // don't question it,.....
            // it's how the X gods want it to be.
            // left, right, top, bottom, left_start_y, left_end_y, right_start_y, right_end_y, top_start_x, top_end_x, bottom_start_x, bottom_end_x
            #[rustfmt::skip]
            let strut_list: Vec<u8> = match strut_def.side {
                Side::Left   => vec![dist + monitor_rect.x as u32,  0,                                                     0,                             0,                                                      monitor_rect.y as u32,  mon_end_y,  0,                      0,          0,                      0,          0,                      0],
                Side::Right  => vec![0,                             root_window_geometry.width as u32 - mon_end_x + dist,  0,                             0,                                                      0,                      0,          monitor_rect.y as u32,  mon_end_y,  0,                      0,          0,                      0],
                Side::Top    => vec![0,                             0,                                                     dist + monitor_rect.y as u32,  0,                                                      0,                      0,          0,                      0,          monitor_rect.x as u32,  mon_end_x,  0,                      0],
                Side::Bottom => vec![0,                             0,                                                     0,                             root_window_geometry.height as u32 - mon_end_y + dist,  0,                      0,          0,                      0,          0,                      0,          monitor_rect.x as u32,  mon_end_x],
                // This should never happen but if it does the window will be anchored on the
                // right of the screen
                _  => vec![0,                             root_window_geometry.width as u32 - mon_end_x + dist,  0,                             0,                                                      0,                      0,          monitor_rect.y as u32,  mon_end_y,  0,                      0,          0,                      0],
            }.iter().flat_map(|x| x.to_le_bytes().to_vec()).collect();

            self.conn
                .change_property(
                    PropMode::REPLACE,
                    win_id,
                    self.atoms._NET_WM_STRUT,
                    self.atoms.CARDINAL,
                    32,
                    4,
                    &strut_list[0..16],
                )?
                .check()?;
            self.conn
                .change_property(
                    PropMode::REPLACE,
                    win_id,
                    self.atoms._NET_WM_STRUT_PARTIAL,
                    self.atoms.CARDINAL,
                    32,
                    12,
                    &strut_list,
                )?
                .check()?;
            self.conn.flush()?;
            Ok(())
        }
    }

    x11rb::atom_manager! {
        pub AtomCollection: AtomCollectionCookie {
            _NET_WM_WINDOW_TYPE,
            _NET_WM_WINDOW_TYPE_DOCK,
            _NET_WM_WINDOW_TYPE_DIALOG,
            _NET_WM_STATE,
            _NET_WM_STATE_STICKY,
            _NET_WM_STATE_ABOVE,
            _NET_WM_STATE_BELOW,
            _NET_WM_NAME,
            _NET_WM_STRUT,
            _NET_WM_STRUT_PARTIAL,
            WM_NAME,
            UTF8_STRING,
            COMPOUND_TEXT,
            CARDINAL,
            ATOM,
            WM_CLASS,
            STRING,
        }
    }
}
