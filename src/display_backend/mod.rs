use anyhow::*;

#[cfg(not(target_os = "macos"))]
pub mod x11;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MonitorName(String);

impl MonitorName {
    pub fn get_name(&self) -> &String {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StackingStrategy {
    AlwaysOnTop,
    AlwaysOnBottom,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MonitorData {
    port_name: MonitorName,
    primary: bool,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

pub trait DisplayBackend {
    type WinId: Copy;

    fn get_monitors(&self) -> Result<Vec<MonitorData>>;
    fn get_primary_monitor(&self) -> Result<MonitorData>;

    fn place_window_at(&self, win: Self::WinId, x: i32, y: i32) -> Result<()>;
    fn resize_window(&self, win: Self::WinId, width: u32, height: u32) -> Result<()>;
    fn set_stacking_strategy(&self, win: Self::WinId, strategy: StackingStrategy) -> Result<()>;
    fn set_as_dock(&self, win: Self::WinId) -> Result<()>;
    fn set_application_id<S: AsRef<str>>(&self, win: Self::WinId, id: S) -> Result<()>;
    fn set_window_title<S: AsRef<str>>(&self, win: Self::WinId, title: S) -> Result<()>;
    fn get_window_id_of(&self, window: &gtk4::Window) -> Self::WinId;

    fn get_monitor(&self, name: &MonitorName) -> Result<MonitorData> {
        self.get_monitors()?
            .into_iter()
            .find(|m| &m.port_name == name)
            .context(format!("No monitor named {} found", name.get_name()))
    }
}

#[cfg(target_os = "macos")]
pub fn get_backend() -> Result<impl DisplayBackend> {
    unimplemented!()
}
#[cfg(not(target_os = "macos"))]
pub fn get_backend() -> Result<impl DisplayBackend> {
    x11::X11Backend::new()
}
