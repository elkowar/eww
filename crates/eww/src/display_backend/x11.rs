use anyhow::{Context, Result};
use gdkx11;
use gtk::{self, prelude::*};
use itertools::Itertools;
use x11rb::protocol::xproto::ConnectionExt;

// see https://github.com/dancor/wmctrl/blob/master/main.c
const MAX_PROPERTY_VALUE_LEN: u32 = 4096;

use x11rb::{
    self,
    connection::Connection,
    protocol::xproto::*,
    rust_connection::{DefaultStream, RustConnection},
};
use yuck::config::{
    backend_window_options::{Side, WindowType},
    window_definition::{WindowDefinition, WindowStacking},
};

pub fn initialize_window(window_def: &WindowDefinition, _monitor: gdk::Rectangle) -> Option<gtk::Window> {
    let window_type = if window_def.backend_options.wm_ignore { gtk::WindowType::Popup } else { gtk::WindowType::Toplevel };
    let window = gtk::Window::new(window_type);
    let wm_class_name = format!("eww-{}", window_def.name);
    #[allow(deprecated)]
    window.set_wmclass(&wm_class_name, &wm_class_name);
    window.set_resizable(window_def.resizable);
    window.set_keep_above(window_def.stacking == WindowStacking::Foreground);
    window.set_keep_below(window_def.stacking == WindowStacking::Background);
    if window_def.backend_options.sticky {
        window.stick();
    } else {
        window.unstick();
    }
    Some(window)
}

pub fn set_xprops(window: &gtk::Window, monitor: gdk::Rectangle, window_def: &WindowDefinition) -> Result<()> {
    let backend = X11Backend::new()?;
    backend.set_xprops_for(window, monitor, window_def)?;
    Ok(())
}

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
        Ok(X11Backend { conn, root_window: screen.root, atoms })
    }

    fn set_xprops_for(&self, window: &gtk::Window, monitor_rect: gdk::Rectangle, window_def: &WindowDefinition) -> Result<()> {
        let gdk_window = window.window().context("Couldn't get gdk window from gtk window")?;
        let win_id =
            gdk_window.downcast_ref::<gdkx11::X11Window>().context("Failed to get x11 window for gtk window")?.xid() as u32;
        let strut_def = window_def.backend_options.struts;
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
                Side::Bottom => vec![0,                             0,                                                     0,                             root_window_geometry.height as u32 - mon_end_y + dist,  0,                      0,          0,                      0,          0,                      0,          monitor_rect.x as u32,  mon_end_x],
                // This should never happen but if it does the window will be anchored on the
                // right of the screen
            }.iter().flat_map(|x| x.to_le_bytes().to_vec()).collect();

        self.conn
            .change_property(PropMode::REPLACE, win_id, self.atoms._NET_WM_STRUT, self.atoms.CARDINAL, 32, 4, &strut_list[0..16])?
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
            &[match window_def.backend_options.window_type {
                WindowType::Dock => self.atoms._NET_WM_WINDOW_TYPE_DOCK,
                WindowType::Normal => self.atoms._NET_WM_WINDOW_TYPE_NORMAL,
                WindowType::Dialog => self.atoms._NET_WM_WINDOW_TYPE_DIALOG,
                WindowType::Toolbar => self.atoms._NET_WM_WINDOW_TYPE_TOOLBAR,
                WindowType::Utility => self.atoms._NET_WM_WINDOW_TYPE_UTILITY,
                WindowType::Desktop => self.atoms._NET_WM_WINDOW_TYPE_DESKTOP,
                WindowType::Notification => self.atoms._NET_WM_WINDOW_TYPE_NOTIFICATION,
            }],
        )?
        .check()?;

        self.conn.flush().context("Failed to send requests to X server")
    }

    pub fn get_workspace_data(&self) -> Result<Vec<WorkspaceData>> {
        let active_workspace = self.conn.get_property(
            false,
            self.root_window,
            self.atoms._NET_CURRENT_DESKTOP,
            self.atoms.CARDINAL,
            0,
            MAX_PROPERTY_VALUE_LEN / 4,
        )?;
        let active_workspace =
            active_workspace.reply()?.value32().context("Got wrong atom format")?.next().context("Got no value")?;
        let workspace_list = self.conn.get_property(
            false,
            self.root_window,
            self.atoms._NET_DESKTOP_NAMES,
            self.atoms.UTF8_STRING,
            0,
            MAX_PROPERTY_VALUE_LEN / 4,
        )?;
        let workspace_list: Vec<u8> = workspace_list.reply()?.value8().context("Got wrong atom format")?.collect();
        let elements = String::from_utf8(workspace_list)?
            .split('\0')
            .dropping_back(1)
            .enumerate()
            .map(|(i, elem)| WorkspaceData { name: elem.to_string(), index: i, active: active_workspace as usize == i })
            .collect::<Vec<_>>();
        Ok(elements)
    }
}

#[derive(Debug)]
pub struct WorkspaceData {
    name: String,
    index: usize,
    active: bool,
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
        _NET_CURRENT_DESKTOP,
        _NET_DESKTOP_NAMES,
        WM_NAME,
        UTF8_STRING,
        COMPOUND_TEXT,
        CARDINAL,
        ATOM,
        WM_CLASS,
        STRING,
    }
}
