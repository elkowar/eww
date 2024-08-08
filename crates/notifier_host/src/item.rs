use crate::*;

use gtk::{self, prelude::*};

/// Recognised values of [`org.freedesktop.StatusNotifierItem.Status`].
///
/// [`org.freedesktop.StatusNotifierItem.Status`]: https://www.freedesktop.org/wiki/Specifications/StatusNotifierItem/StatusNotifierItem/#org.freedesktop.statusnotifieritem.status
#[derive(Debug, Clone, Copy)]
pub enum Status {
    /// The item doesn't convey important information to the user, it can be considered an "idle"
    /// status and is likely that visualizations will chose to hide it.
    Passive,
    /// The item is active, is more important that the item will be shown in some way to the user.
    Active,
    /// The item carries really important information for the user, such as battery charge running
    /// out and is wants to incentive the direct user intervention. Visualizations should emphasize
    /// in some way the items with NeedsAttention status.
    NeedsAttention,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParseStatusError;

impl std::str::FromStr for Status {
    type Err = ParseStatusError;

    fn from_str(s: &str) -> std::result::Result<Self, ParseStatusError> {
        match s {
            "Passive" => Ok(Status::Passive),
            "Active" => Ok(Status::Active),
            "NeedsAttention" => Ok(Status::NeedsAttention),
            _ => Err(ParseStatusError),
        }
    }
}

/// A StatusNotifierItem (SNI).
///
/// At the moment, this does not wrap much of the SNI's properties and methods. As such, you should
/// directly access the `sni` member as needed for functionalty that is not provided.
pub struct Item {
    /// The StatusNotifierItem that is wrapped by this instance.
    pub sni: proxy::StatusNotifierItemProxy<'static>,
    gtk_menu: Option<dbusmenu_gtk3::Menu>,
}

impl Item {
    /// Create an instance from the service's address.
    ///
    /// The format of `addr` is `{bus}{object_path}` (e.g.
    /// `:1.50/org/ayatana/NotificationItem/nm_applet`), which is the format that is used for
    /// StatusNotifierWatcher's [RegisteredStatusNotifierItems property][rsni]).
    ///
    /// [rsni]: https://freedesktop.org/wiki/Specifications/StatusNotifierItem/StatusNotifierWatcher/#registeredstatusnotifieritems
    pub async fn from_address(con: &zbus::Connection, service: &str) -> zbus::Result<Self> {
        let (addr, path) = {
            // Based on <https://github.com/oknozor/stray/blob/main/stray/src/notifier_watcher/notifier_address.rs>
            //
            // TODO is the service name format actually documented anywhere?
            if let Some((addr, path)) = service.split_once('/') {
                (addr.to_owned(), format!("/{}", path))
            } else if service.starts_with(':') {
                (service[0..6].to_owned(), names::ITEM_OBJECT.to_owned())
            } else {
                return Err(zbus::Error::Address(service.to_owned()));
            }
        };

        let sni = proxy::StatusNotifierItemProxy::builder(con).destination(addr)?.path(path)?.build().await?;

        Ok(Self { sni, gtk_menu: None })
    }

    /// Get the current status of the item.
    pub async fn status(&self) -> zbus::Result<Status> {
        let status = self.sni.status().await?;
        match status.parse() {
            Ok(s) => Ok(s),
            Err(_) => Err(zbus::Error::Failure(format!("Invalid status {:?}", status))),
        }
    }

    pub async fn set_menu(&mut self, widget: &gtk::EventBox) -> zbus::Result<()> {
        let menu = dbusmenu_gtk3::Menu::new(self.sni.inner().destination(), &self.sni.menu().await?);
        menu.set_attach_widget(Some(widget));
        self.gtk_menu = Some(menu);
        Ok(())
    }

    pub async fn popup_menu(&self, event: &gtk::gdk::EventButton, x: i32, y: i32) -> zbus::Result<()> {
        if let Some(menu) = &self.gtk_menu {
            menu.popup_at_pointer(event.downcast_ref::<gtk::gdk::Event>());
            Ok(())
        } else {
            self.sni.context_menu(x, y).await
        }
    }

    /// Get the current icon.
    pub async fn icon(&self, size: i32, scale: i32) -> Option<gtk::gdk_pixbuf::Pixbuf> {
        // TODO explain what size and scale mean here

        // see icon.rs
        load_icon_from_sni(&self.sni, size, scale).await
    }
}
