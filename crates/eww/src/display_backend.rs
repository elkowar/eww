use crate::window_initiator::WindowInitiator;

#[cfg(feature = "wayland")]
pub use platform_wayland::WaylandBackend;

#[cfg(feature = "x11")]
pub use platform_x11::{set_xprops, X11Backend};

pub trait DisplayBackend: Send + Sync + 'static {
    const IS_X11: bool;

    fn initialize_window(window_init: &WindowInitiator, monitor: gdk::Rectangle) -> Option<gtk::Window>;
}

pub struct NoBackend;

impl DisplayBackend for NoBackend {
    const IS_X11: bool = false;

    fn initialize_window(_window_init: &WindowInitiator, _monitor: gdk::Rectangle) -> Option<gtk::Window> {
        Some(gtk::Window::new(gtk::WindowType::Toplevel))
    }
}

#[cfg(feature = "wayland")]
mod platform_wayland {
    use crate::window_initiator::WindowInitiator;
    use gtk::prelude::*;
    use yuck::config::{window_definition::WindowStacking, window_geometry::AnchorAlignment};

    use super::DisplayBackend;

    pub struct WaylandBackend;

    impl DisplayBackend for WaylandBackend {
        const IS_X11: bool = false;

        fn initialize_window(window_init: &WindowInitiator, monitor: gdk::Rectangle) -> Option<gtk::Window> {
            let window = gtk::Window::new(gtk::WindowType::Toplevel);
            // Initialising a layer shell surface
            gtk_layer_shell::init_for_window(&window);
            // Sets the monitor where the surface is shown
            if let Some(ident) = window_init.monitor.clone() {
                let display = gdk::Display::default().expect("could not get default display");
                if let Some(monitor) = crate::app::get_monitor_from_display(&display, &ident) {
                    gtk_layer_shell::set_monitor(&window, &monitor);
                } else {
                    return None;
                }
            };
            window.set_resizable(window_init.resizable);

            // Sets the layer where the layer shell surface will spawn
            match window_init.stacking {
                WindowStacking::Foreground => gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Top),
                WindowStacking::Background => gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Background),
                WindowStacking::Bottom => gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Bottom),
                WindowStacking::Overlay => gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Overlay),
            }

            if let Some(namespace) = &window_init.backend_options.wayland.namespace {
                gtk_layer_shell::set_namespace(&window, namespace);
            }

            // Sets the keyboard interactivity
            gtk_layer_shell::set_keyboard_interactivity(&window, window_init.backend_options.wayland.focusable);

            if let Some(geometry) = window_init.geometry {
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

                let xoffset = geometry.offset.x.pixels_relative_to(monitor.width());
                let yoffset = geometry.offset.y.pixels_relative_to(monitor.height());

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
            if window_init.backend_options.wayland.exclusive {
                gtk_layer_shell::auto_exclusive_zone_enable(&window);
            }
            Some(window)
        }
    }
}

#[cfg(feature = "x11")]
mod platform_x11 {
    use crate::window_initiator::WindowInitiator;
    use anyhow::{Context, Result};
    use gdk::Monitor;
    use gtk::{self, prelude::*};
    use x11rb::protocol::xproto::ConnectionExt;

    use x11rb::{
        self,
        connection::Connection,
        protocol::xproto::*,
        rust_connection::{DefaultStream, RustConnection},
    };
    use yuck::config::{
        backend_window_options::{Side, X11WindowType},
        window_definition::WindowStacking,
    };

    use super::DisplayBackend;

    pub struct X11Backend;
    impl DisplayBackend for X11Backend {
        const IS_X11: bool = true;

        fn initialize_window(window_init: &WindowInitiator, _monitor: gdk::Rectangle) -> Option<gtk::Window> {
            let window_type =
                if window_init.backend_options.x11.wm_ignore { gtk::WindowType::Popup } else { gtk::WindowType::Toplevel };
            let window = gtk::Window::new(window_type);
            window.set_resizable(window_init.resizable);
            window.set_keep_above(window_init.stacking == WindowStacking::Foreground);
            window.set_keep_below(window_init.stacking == WindowStacking::Background);
            if window_init.backend_options.x11.sticky {
                window.stick();
            } else {
                window.unstick();
            }
            Some(window)
        }
    }

    pub fn set_xprops(window: &gtk::Window, monitor: Monitor, window_init: &WindowInitiator) -> Result<()> {
        let backend = X11BackendConnection::new()?;
        backend.set_xprops_for(window, monitor, window_init)?;
        Ok(())
    }

    struct X11BackendConnection {
        conn: RustConnection<DefaultStream>,
        root_window: u32,
        atoms: AtomCollection,
    }

    impl X11BackendConnection {
        fn new() -> Result<Self> {
            let (conn, screen_num) = RustConnection::connect(None)?;
            let screen = conn.setup().roots[screen_num].clone();
            let atoms = AtomCollection::new(&conn)?.reply()?;
            Ok(X11BackendConnection { conn, root_window: screen.root, atoms })
        }

        fn set_xprops_for(&self, window: &gtk::Window, monitor: Monitor, window_init: &WindowInitiator) -> Result<()> {
            let monitor_rect = monitor.geometry();
            let scale_factor = monitor.scale_factor() as u32;
            let gdk_window = window.window().context("Couldn't get gdk window from gtk window")?;
            let win_id =
                gdk_window.downcast_ref::<gdkx11::X11Window>().context("Failed to get x11 window for gtk window")?.xid() as u32;
            let strut_def = window_init.backend_options.x11.struts;
            let root_window_geometry = self.conn.get_geometry(self.root_window)?.reply()?;

            let mon_x = scale_factor * monitor_rect.x() as u32;
            let mon_y = scale_factor * monitor_rect.y() as u32;
            let mon_end_x = scale_factor * (monitor_rect.x() + monitor_rect.width()) as u32 - 1u32;
            let mon_end_y = scale_factor * (monitor_rect.y() + monitor_rect.height()) as u32 - 1u32;

            let dist = match strut_def.side {
                Side::Left | Side::Right => strut_def.distance.pixels_relative_to(monitor_rect.width()) as u32,
                Side::Top | Side::Bottom => strut_def.distance.pixels_relative_to(monitor_rect.height()) as u32,
            };

            // don't question it,.....
            // it's how the X gods want it to be.
            // left, right, top, bottom, left_start_y, left_end_y, right_start_y, right_end_y, top_start_x, top_end_x, bottom_start_x, bottom_end_x
            #[rustfmt::skip]
            let strut_list: Vec<u8> = match strut_def.side {
                Side::Left   => vec![dist + mon_x, 0,                                                    0,                   0,                                                     mon_x, mon_end_y, 0,     0,         0,     0,         0,             0],
                Side::Right  => vec![0,            root_window_geometry.width as u32 - mon_end_x + dist, 0,                   0,                                                     0,     0,         mon_x, mon_end_y, 0,     0,         0,             0],
                Side::Top    => vec![0,            0,                                                    dist + mon_y as u32, 0,                                                     0,     0,         0,     0,         mon_x, mon_end_x, 0,             0],
                Side::Bottom => vec![0,            0,                                                    0,                   root_window_geometry.height as u32 - mon_end_y + dist, 0,     0,         0,     0,         0,     0,         mon_x as u32,  mon_end_x],
                // This should never happen but if it does the window will be anchored on the
                // right of the screen
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

            // TODO possibly support setting multiple window types
            x11rb::wrapper::ConnectionExt::change_property32(
                &self.conn,
                PropMode::REPLACE,
                win_id,
                self.atoms._NET_WM_WINDOW_TYPE,
                self.atoms.ATOM,
                &[match window_init.backend_options.x11.window_type {
                    X11WindowType::Dock => self.atoms._NET_WM_WINDOW_TYPE_DOCK,
                    X11WindowType::Normal => self.atoms._NET_WM_WINDOW_TYPE_NORMAL,
                    X11WindowType::Dialog => self.atoms._NET_WM_WINDOW_TYPE_DIALOG,
                    X11WindowType::Toolbar => self.atoms._NET_WM_WINDOW_TYPE_TOOLBAR,
                    X11WindowType::Utility => self.atoms._NET_WM_WINDOW_TYPE_UTILITY,
                    X11WindowType::Desktop => self.atoms._NET_WM_WINDOW_TYPE_DESKTOP,
                    X11WindowType::Notification => self.atoms._NET_WM_WINDOW_TYPE_NOTIFICATION,
                }],
            )?
            .check()?;

            self.conn.flush().context("Failed to send requests to X server")
        }
    }

    x11rb::atom_manager! {
        pub AtomCollection: AtomCollectionCookie {
            _NET_WM_WINDOW_TYPE,
            _NET_WM_WINDOW_TYPE_NORMAL,
            _NET_WM_WINDOW_TYPE_DOCK,
            _NET_WM_WINDOW_TYPE_DIALOG,
            _NET_WM_WINDOW_TYPE_TOOLBAR,
            _NET_WM_WINDOW_TYPE_UTILITY,
            _NET_WM_WINDOW_TYPE_DESKTOP,
            _NET_WM_WINDOW_TYPE_NOTIFICATION,
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
