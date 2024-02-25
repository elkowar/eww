//! Proxies for DBus services, so we can call them.
//!
//! The interface XML files were taken from
//! [Waybar](https://github.com/Alexays/Waybar/tree/master/protocol), and the proxies were
//! generated with [zbus-xmlgen](https://docs.rs/crate/zbus_xmlgen/latest) by running `zbus-xmlgen
//! dbus_status_notifier_item.xml` and `zbus-xmlgen dbus_status_notifier_watcher.xml`. At the
//! moment, `dbus_menu.xml` isn't used.
//!
//! For more information, see ["Writing a client proxy" in the zbus
//! tutorial](https://dbus2.github.io/zbus/).

mod dbus_status_notifier_item;
pub use dbus_status_notifier_item::*;

mod dbus_status_notifier_watcher;
pub use dbus_status_notifier_watcher::*;
