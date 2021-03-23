pub use platform::*;

#[cfg(feature = "no-x11")]
mod platform {
    use crate::config::{Side, StrutDefinition};
    use anyhow::*;
    pub fn reserve_space_for(window: &gtk::Window, monitor: gdk::Rectangle, strut_def: StrutDefinition) -> Result<()> {
        Err(anyhow!("Cannot reserve space on non-X11 backends"))
    }
}

#[cfg(feature = "x11")]
mod platform {
    use crate::config::{Side, StrutDefinition};
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
            };

            // don't question it,.....
            // it's how the X gods want it to be.
            // left, right, top, bottom, left_start_y, left_end_y, right_start_y, right_end_y, top_start_x, top_end_x, bottom_start_x, bottom_end_x
            #[rustfmt::skip]
            let strut_list: Vec<u8> = match strut_def.side {
                Side::Left   => vec![dist + monitor_rect.x as u32,  0,                                                     0,                             0,                                                      monitor_rect.y as u32,  mon_end_y,  0,                      0,          0,                      0,          0,                      0],
                Side::Right  => vec![0,                             root_window_geometry.width as u32 - mon_end_x + dist,  0,                             0,                                                      0,                      0,          monitor_rect.y as u32,  mon_end_y,  0,                      0,          0,                      0],
                Side::Top    => vec![0,                             0,                                                     dist + monitor_rect.y as u32,  0,                                                      0,                      0,          0,                      0,          monitor_rect.x as u32,  mon_end_x,  0,                      0],
                Side::Bottom => vec![0,                             0,                                                     0,                             root_window_geometry.height as u32 - mon_end_y + dist,  0,                      0,          0,                      0,          0,                      0,          monitor_rect.x as u32,  mon_end_x]
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
