#[cfg(not(any(feature = "x11", feature = "wayland")))]
mod no_backend {
    use yuck::config::window_definition::{WindowDefinition, WindowStacking};

    pub fn initialize_window(_window_def: &WindowDefinition, _monitor: gdk::Rectangle) -> Option<gtk::Window> {
        Some(gtk::Window::new(gtk::WindowType::Toplevel))
    }
}
#[cfg(not(any(feature = "x11", feature = "wayland")))]
pub use no_backend::*;

#[cfg(feature = "wayland")]
mod wayland;
#[cfg(feature = "wayland")]
pub use wayland::*;

#[cfg(feature = "x11")]
mod x11;
#[cfg(feature = "x11")]
pub use x11::*;
