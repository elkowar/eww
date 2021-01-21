use super::*;
use gdk4_x11;
use gtk4::{self, prelude::*, GtkWindowExt};
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

    fn get_monitors(&self) -> Result<Vec<MonitorData>> {
        randr::get_monitors(&self.conn, self.root_window, false)?
            .reply()?
            .monitors
            .into_iter()
            .map(|info| {
                let name_res = self.conn.get_atom_name(info.name)?.reply()?;
                let name = String::from_utf8(name_res.name)?;

                Ok(MonitorData {
                    x: info.x as i32,
                    y: info.y as i32,
                    width: info.width as u32,
                    height: info.height as u32,
                    primary: info.primary,
                    port_name: MonitorName(name),
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
        self.conn.configure_window(
            win,
            &ConfigureWindowAux {
                x: Some(x),
                y: Some(y),
                ..ConfigureWindowAux::default()
            },
        )?;
        self.conn.flush()?;
        Ok(())
    }

    fn resize_window(&self, win: Self::WinId, width: u32, height: u32) -> Result<()> {
        self.conn.configure_window(
            win,
            &ConfigureWindowAux {
                width: Some(width as u32),
                height: Some(height as u32),
                ..ConfigureWindowAux::default()
            },
        )?;
        self.conn.flush()?;
        Ok(())
    }

    fn set_as_dock(&self, win: Self::WinId) -> Result<()> {
        self.conn.change_property(
            PropMode::Replace,
            win,
            self.atoms._NET_WM_WINDOW_TYPE,
            self.atoms.ATOM,
            32,
            1,
            &self.atoms._NET_WM_WINDOW_TYPE_DOCK.to_le_bytes(),
        )?;

        // self.conn.change_window_attributes(
        // win,
        //&ChangeWindowAttributesAux {
        // override_redirect: Some(true.into()),
        //..ChangeWindowAttributesAux::default()
        //},
        //)?;
        self.conn.flush()?;
        Ok(())
    }

    fn set_stacking_strategy(&self, win: Self::WinId, strategy: StackingStrategy) -> Result<()> {
        self.conn.configure_window(
            win,
            &ConfigureWindowAux {
                stack_mode: Some(match strategy {
                    StackingStrategy::AlwaysOnTop => StackMode::Above,
                    StackingStrategy::AlwaysOnBottom => StackMode::Below,
                }),
                ..ConfigureWindowAux::default()
            },
        )?;
        self.conn.flush()?;
        Ok(())
    }

    fn set_window_title<S: AsRef<str>>(&self, win: Self::WinId, id: S) -> Result<()> {
        let bytes = id.as_ref().as_bytes();
        self.conn.change_property(
            PropMode::Replace,
            win,
            self.atoms._NET_WM_NAME,
            self.atoms.UTF8_STRING,
            8,
            bytes.len() as u32,
            bytes,
        )?;
        self.conn.change_property(
            PropMode::Replace,
            win,
            self.atoms.WM_NAME,
            self.atoms.COMPOUND_TEXT,
            8,
            bytes.len() as u32,
            bytes,
        )?;
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

        self.conn.change_property(
            PropMode::Replace,
            win,
            self.atoms.WM_CLASS,
            self.atoms.STRING,
            8,
            bytes.len() as u32,
            &bytes,
        )?;
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
        _NET_WM_NAME,
        WM_NAME,
        UTF8_STRING,
        COMPOUND_TEXT,
        ATOM,
        WM_CLASS,
        STRING,
    }
}
