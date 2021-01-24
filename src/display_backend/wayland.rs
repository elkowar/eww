pub use crate::display_backend::*;
pub struct WaylandBackend;

impl WaylandBackend {
    pub fn new() -> Result<Self> {
        Ok(WaylandBackend)
    }
}

impl DisplayBackend for WaylandBackend {
    type WinId = ();

    fn get_monitors(&self) -> Result<Vec<MonitorData>> {
        Ok(Vec::new())
    }

    fn get_primary_monitor(&self) -> Result<MonitorData> {
        // TODO this is kinda ugly
        Ok(MonitorData {
            port_name: "unknown monitor".to_string(),
            primary: true,
            rect: Rect::of(0, 0, 0, 0),
        })
    }

    fn map_window(&self, win: Self::WinId) -> Result<()> {
        Ok(())
    }

    fn place_window_at(&self, win: Self::WinId, x: i32, y: i32) -> Result<()> {
        Ok(())
    }

    fn resize_window(&self, win: Self::WinId, width: u32, height: u32) -> Result<()> {
        Ok(())
    }

    fn set_stacking_strategy(&self, win: Self::WinId, strategy: StackingStrategy) -> Result<()> {
        Ok(())
    }

    fn set_as_dock(&self, win: Self::WinId) -> Result<()> {
        Ok(())
    }

    fn set_unmanaged(&self, win: Self::WinId) -> Result<()> {
        Ok(())
    }

    fn set_application_id<S: AsRef<str>>(&self, win: Self::WinId, id: S) -> Result<()> {
        Ok(())
    }

    fn get_window_id_of(&self, window: &gtk4::Window) -> Self::WinId {
        ()
    }

    fn get_monitor(&self, name: &str) -> Result<MonitorData> {
        Ok(MonitorData {
            port_name: "unknown monitor".to_string(),
            primary: true,
            rect: Rect::of(0, 0, 0, 0),
        })
    }
}
