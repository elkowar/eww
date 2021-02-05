use super::*;
use gdk4_x11;
use gtk4::{self, prelude::*};
use x11rb::protocol::xproto::ConnectionExt;

use x11rb::{
    self,
    connection::Connection,
    protocol::{randr, xproto::*},
    rust_connection::{DefaultStream, RustConnection},
};

pub struct X11Backend {
    conn: RustConnection<DefaultStream>,
    root_window: u32,
    atoms: AtomCollection,
}

impl X11Backend {
    pub fn new() -> Result<Self> {
        let (conn, screen_num) = RustConnection::connect(None)?;
        let screen = conn.setup().roots[screen_num].clone();
        let atoms = AtomCollection::new(&conn)?.reply()?;
        Ok(X11Backend {
            conn,
            root_window: screen.root,
            atoms,
        })
    }
}

impl DisplayBackend for X11Backend {
    type WinId = u32;

    fn map_window(&self, win: Self::WinId) -> Result<()> {
        self.conn.map_window(win)?.check()?;
        Ok(())
    }

    fn get_monitors(&self) -> Result<Vec<MonitorData>> {
        randr::get_monitors(&self.conn, self.root_window, false)?
            .reply()?
            .monitors
            .into_iter()
            .map(|info| {
                let name_res = self.conn.get_atom_name(info.name)?.reply()?;
                let name = String::from_utf8(name_res.name)?;
                Ok(MonitorData {
                    rect: Rect {
                        x: info.x as i32,
                        y: info.y as i32,
                        width: info.width as i32,
                        height: info.height as i32,
                    },
                    primary: info.primary,
                    port_name: name,
                })
            })
            .collect()
    }

    fn get_primary_monitor(&self) -> Result<MonitorData> {
        let monitors = self.get_monitors()?;
        monitors
            .iter()
            .find(|m| m.primary)
            .cloned()
            .or_else(|| monitors.first().cloned())
            .context(format!("No monitors found"))
    }

    // TODO monitor
    fn place_window_at(&self, win: Self::WinId, x: i32, y: i32) -> Result<()> {
        self.conn
            .configure_window(
                win,
                &ConfigureWindowAux {
                    x: Some(x),
                    y: Some(y),
                    ..ConfigureWindowAux::default()
                },
            )?
            .check()?;
        self.conn.flush()?;
        Ok(())
    }

    fn resize_window(&self, win: Self::WinId, width: u32, height: u32) -> Result<()> {
        self.conn
            .configure_window(
                win,
                &ConfigureWindowAux {
                    width: Some(width as u32),
                    height: Some(height as u32),
                    ..ConfigureWindowAux::default()
                },
            )?
            .check()?;
        self.conn.flush()?;
        Ok(())
    }

    fn set_unmanaged(&self, win: Self::WinId) -> Result<()> {
        self.conn
            .change_property(
                PropMode::Replace,
                win,
                self.atoms._NET_WM_STATE,
                self.atoms.ATOM,
                32,
                1,
                &self.atoms._NET_WM_STATE_STICKY.to_le_bytes(),
            )?
            .check()?;

        self.conn
            .change_window_attributes(
                win,
                &ChangeWindowAttributesAux {
                    override_redirect: Some(true.into()),
                    ..ChangeWindowAttributesAux::default()
                },
            )?
            .check()?;

        self.conn.flush()?;
        Ok(())
    }

    fn set_as_dock(&self, win: Self::WinId) -> Result<()> {
        self.conn
            .change_property(
                PropMode::Replace,
                win,
                self.atoms._NET_WM_WINDOW_TYPE,
                self.atoms.ATOM,
                32,
                1,
                &self.atoms._NET_WM_WINDOW_TYPE_DOCK.to_le_bytes(),
            )?
            .check()?;

        self.conn.flush()?;
        Ok(())
    }

    fn set_stacking_strategy(&self, win: Self::WinId, strategy: StackingStrategy) -> Result<()> {
        let (stack_mode, stacking_wm_state) = match strategy {
            StackingStrategy::AlwaysOnTop => (StackMode::Above, self.atoms._NET_WM_STATE_ABOVE),
            StackingStrategy::AlwaysOnBottom => (StackMode::Below, self.atoms._NET_WM_STATE_ABOVE),
        };

        self.conn
            .configure_window(
                win,
                &ConfigureWindowAux {
                    stack_mode: Some(stack_mode),
                    ..ConfigureWindowAux::default()
                },
            )?
            .check()?;

        self.conn
            .change_property(
                PropMode::Append,
                win,
                self.atoms._NET_WM_STATE,
                self.atoms.ATOM,
                32,
                1,
                &stacking_wm_state.to_le_bytes(),
            )?
            .check()?;

        self.conn.flush()?;

        Ok(())
    }

    fn set_application_id<S: AsRef<str>>(&self, win: Self::WinId, id: S) -> Result<()> {
        let bytes = {
            let mut bytes = id.as_ref().as_bytes().to_vec();
            bytes.push(0);
            let mut bytes_again = id.as_ref().as_bytes().to_vec();
            bytes.append(&mut bytes_again);
            bytes
        };

        self.conn
            .change_property(
                PropMode::Replace,
                win,
                self.atoms.WM_CLASS,
                self.atoms.STRING,
                8,
                bytes.len() as u32,
                &bytes,
            )?
            .check()?;
        self.conn.flush()?;
        Ok(())
    }

    fn reserve_space(&self, win: Self::WinId, monitor: &Option<String>, strut_def: StrutDefinition) -> Result<()> {
        let monitor = match monitor {
            Some(monitor) => self.get_monitor(monitor)?,
            None => self.get_primary_monitor()?,
        };
        let monitor_rect = monitor.get_rect();

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
                PropMode::Replace,
                win,
                self.atoms._NET_WM_STRUT,
                self.atoms.CARDINAL,
                32,
                4,
                &strut_list[0..16],
            )?
            .check()?;
        self.conn
            .change_property(
                PropMode::Replace,
                win,
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

    fn get_window_id_of(&self, window: &gtk4::Window) -> Self::WinId {
        window
            .get_surface()
            .unwrap()
            .downcast::<gdk4_x11::X11Surface>()
            .expect("Not a X11 surface")
            .get_xid() as u32
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
