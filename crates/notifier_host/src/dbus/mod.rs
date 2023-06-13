//! # DBus interface proxies
//!
//! The interface XML files are taken from
//! [Waybar](https://github.com/Alexays/Waybar/tree/master/protocol), and the proxies generated
//! with [zbus-gen](https://docs.rs/crate/zbus_xmlgen/latest).

mod dbus_status_notifier_item;
pub use dbus_status_notifier_item::*;

mod dbus_status_notifier_watcher;
pub use dbus_status_notifier_watcher::*;
