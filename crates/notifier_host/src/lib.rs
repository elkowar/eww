//! The system tray side of the [notifier host DBus
//! protocols](https://freedesktop.org/wiki/Specifications/StatusNotifierItem/StatusNotifierHost/),
//! implementing most of the relevant DBus protocol logic so system tray implementations (e.g. eww)
//! don't need to care about them.
//!
//! This crate does not implement the tray icon side of the protocol. For that, see, for example,
//! the [ksni](https://crates.io/crates/ksni) crate.
//!
//! # Overview / Notes for Contributors
//!
//! This crate makes extensive use of the `zbus` library to interact with DBus. You should read
//! through the [zbus tutorial](https://dbus2.github.io/zbus/) if you aren't familiar with DBus or
//! `zbus`.
//!
//! There are two separate services that are required for the tray side of the protocol:
//!
//! - `StatusNotifierWatcher`, a service which tracks what items and trays there are but doesn't do
//!     any rendering. This is implemented by [`Watcher`] (see that for further details), and
//!     should always be started alongside the `StatusNotifierHost`.
//!
//! - `StatusNotifierHost`, the actual tray, which registers itself to the StatusNotifierHost and
//!     subscribes to its signals to know what items exist. This DBus service has a completely
//!     empty interface, but is mainly by StatusNotifierWatcher to know when trays disappear. This
//!     is represented by the [`Host`] trait.
//!
//! The actual tray implements the [`Host`] trait to be notified of when items (called
//! `StatusNotifierItem` in the spec and represented by [`Item`]) appear and disappear, then calls
//! [`run_host`] to run the DBus side of the protocol.
//!
//! If there are multiple trays running on the system, there can be multiple `StatusNotifierHost`s,
//! but only one `StatusNotifierWatcher` (usually from whatever tray was started first).

pub mod proxy;

mod host;
pub use host::*;

mod icon;
pub use icon::*;

mod item;
pub use item::*;

mod watcher;
pub use watcher::*;

pub(crate) mod names {
    pub const WATCHER_BUS: &str = "org.kde.StatusNotifierWatcher";
    pub const WATCHER_OBJECT: &str = "/StatusNotifierWatcher";

    pub const ITEM_OBJECT: &str = "/StatusNotifierItem";
}
