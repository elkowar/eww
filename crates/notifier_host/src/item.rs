use crate::*;

use log;
use gtk::{self, prelude::*};
use zbus::export::ordered_stream::OrderedStreamExt;
use tokio::{sync::watch, select};

/// Recognised values of org.freedesktop.StatusNotifierItem.Status
///
/// See
/// <https://www.freedesktop.org/wiki/Specifications/StatusNotifierItem/StatusNotifierItem/#org.freedesktop.statusnotifieritem.status>
/// for details.
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

/// Split a sevice name e.g. `:1.50:/org/ayatana/NotificationItem/nm_applet` into the address and
/// path.
///
/// Original logic from <https://github.com/oknozor/stray/blob/main/stray/src/notifier_watcher/notifier_address.rs>
fn split_service_name(service: &str) -> zbus::Result<(String, String)> {
    if let Some((addr, path)) = service.split_once('/') {
        Ok((addr.to_owned(), format!("/{}", path)))
    } else if service.contains(':') { // TODO why?
        let addr = service.split(':').skip(1).next();
        // Some StatusNotifierItems will not return an object path, in that case we fallback
        // to the default path.
        if let Some(addr) = addr {
            Ok((addr.to_owned(), "/StatusNotifierItem".to_owned()))
        } else {
            Err(zbus::Error::Address(service.to_owned()))
        }
    } else {
        Err(zbus::Error::Address(service.to_owned()))
    }
}

pub struct Item {
    pub sni: dbus::StatusNotifierItemProxy<'static>,

    status_rx: watch::Receiver<()>,
    title_rx: watch::Receiver<()>,

    task: tokio::task::JoinHandle<()>,
}

impl Drop for Item {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Item {
    pub async fn from_address(con: &zbus::Connection, addr: &str) -> zbus::Result<Self> {
        let (addr, path) = split_service_name(addr)?;
        let sni = dbus::StatusNotifierItemProxy::builder(con)
            .destination(addr)?
            .path(path)?
            .build()
            .await?;
        let sni_out = sni.clone();

        let (status_tx, status_rx) = watch::channel(());
        let (title_tx, title_rx) = watch::channel(());

        let task = tokio::spawn(async move {
            let mut status_updates = sni.receive_new_status().await.unwrap();
            let mut title_updates = sni.receive_new_title().await.unwrap();

            loop {
                select! {
                    _ = status_updates.next() => {
                        status_tx.send_replace(());
                    }
                    _ = title_updates.next() => {
                        title_tx.send_replace(());
                    }
                }
            }
        });

        Ok(Item {
            sni: sni_out,
            status_rx,
            title_rx,
            task,
        })
    }

    /// Get the current status of the item.
    pub async fn status(&self) -> zbus::Result<Status> {
        let status = self.sni.status().await?;
        match status.parse() {
            Ok(s) => Ok(s),
            Err(_) => Err(zbus::Error::Failure(format!("Invalid status {:?}", status))),
        }
    }

    pub fn status_updates(&self) -> watch::Receiver<()> {
        self.status_rx.clone()
    }

    pub async fn title(&self) -> zbus::Result<String> {
        self.sni.title().await
    }

    pub fn title_updates(&self) -> watch::Receiver<()> {
        self.title_rx.clone()
    }

    pub async fn menu(&self) -> zbus::Result<gtk::Menu> {
        // TODO better handling if menu() method doesn't exist
        let menu = dbusmenu_gtk3::Menu::new(self.sni.destination(), &self.sni.menu().await?);
        Ok(menu.upcast())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum IconError {
    #[error("failed to get icon name: {0}")]
    DBusIconName(zbus::Error),
    #[error("failed to get icon theme path: {0}")]
    DBusTheme(zbus::Error),
    #[error("failed to get pixmap: {0}")]
    DBusPixmap(zbus::Error),
    #[error("failed to load icon {icon_name:?} from theme {theme_path:?}")]
    LoadIconFromTheme {
        icon_name: String,
        theme_path: Option<String>,
        source: gtk::glib::Error,
    },
    #[error("no icon available")]
    NotAvailable,
}

/// Get the fallback GTK icon
async fn fallback_icon(size: i32) -> std::result::Result<gtk::gdk_pixbuf::Pixbuf, IconError> {
    // TODO downgrade from panic to error return?
    let theme = gtk::IconTheme::default().expect("Could not get default gtk theme");
    return match theme.load_icon("image-missing", size, gtk::IconLookupFlags::FORCE_SIZE) {
        Err(e) => Err(IconError::LoadIconFromTheme {
            icon_name: "image-missing".to_owned(),
            theme_path: None,
            source: e,
        }),
        Ok(pb) => Ok(pb.unwrap()),
    }
}

impl Item {
    pub fn load_pixbuf(width: i32, height: i32, mut data: Vec<u8>) -> gtk::gdk_pixbuf::Pixbuf {
        // We need to convert data from ARGB32 to RGBA32
        for chunk in data.chunks_mut(4) {
            let a = chunk[0];
            let r = chunk[1];
            let g = chunk[2];
            let b = chunk[3];
            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = a;
        }

        gtk::gdk_pixbuf::Pixbuf::from_bytes(
            &gtk::glib::Bytes::from_owned(data),
            gtk::gdk_pixbuf::Colorspace::Rgb,
            true,
            8,
            width,
            height,
            width * 4,
        )
    }

    async fn icon_from_name(&self, size: i32) -> std::result::Result<gtk::gdk_pixbuf::Pixbuf, IconError> {
        // TODO better handling of icon_name failure instead of punting it to the caller
        let icon_name = match self.sni.icon_name().await {
            Ok(s) if s == "" => return Err(IconError::NotAvailable),
            Ok(s) => s,
            Err(e) => return Err(IconError::DBusIconName(e)),
        };

        let icon_theme_path = match self.sni.icon_theme_path().await {
            Ok(p) if p == "" => None,
            Ok(p) => Some(p),
            Err(zbus::Error::FDO(e)) => match *e {
                zbus::fdo::Error::UnknownProperty(_)
                | zbus::fdo::Error::InvalidArgs(_)
                    => None,
                // this error is reported by discord, blueman-applet
                zbus::fdo::Error::Failed(msg) if msg == "error occurred in Get" => None,
                _ => return Err(IconError::DBusTheme(zbus::Error::FDO(e))),
            },
            Err(e) => return Err(IconError::DBusTheme(e)),
        };

        if let Some(theme_path) = icon_theme_path {
            // icon supplied a theme path, so only look there (w/ fallback)
            let theme = gtk::IconTheme::new();
            theme.prepend_search_path(&theme_path);

            return match theme.load_icon(&icon_name, size, gtk::IconLookupFlags::FORCE_SIZE) {
                Err(e) => Err(IconError::LoadIconFromTheme {
                    icon_name,
                    theme_path: Some(theme_path),
                    source: e,
                }),
                Ok(pb) => return Ok(pb.expect("no pixbuf from theme.load_icon despite no error")),
            }
        }

        // fallback to default theme
        let theme = gtk::IconTheme::default().expect("Could not get default gtk theme");
        match theme.load_icon(&icon_name, size, gtk::IconLookupFlags::FORCE_SIZE) {
            // TODO specifically match on icon missing here
            Err(e) => {
                log::warn!("Could not find icon {:?} in default theme: {}", &icon_name, e);
                Err(IconError::LoadIconFromTheme {
                    icon_name,
                    theme_path: None,
                    source: e,
                })
            },
            Ok(pb) => Ok(pb.unwrap()),
        }
    }

    async fn icon_from_pixmap(&self, size: i32) -> std::result::Result<gtk::gdk_pixbuf::Pixbuf, IconError> {
        match self.sni.icon_pixmap().await {
            Ok(ps) => {
                for (width, height, data) in ps {
                    // TODO use closest size instead of looking for exact match
                    // (can be tested with keepassxc, which only provides 48x48 and 22x22 pixmaps)
                    if width == size && height == size {
                        return Ok(Self::load_pixbuf(width, height, data))
                    }
                }

                Err(IconError::NotAvailable)
            },
            Err(zbus::Error::FDO(e)) => match *e {
                zbus::fdo::Error::UnknownProperty(_)
                | zbus::fdo::Error::InvalidArgs(_)
                    => Err(IconError::NotAvailable),
                _ => Err(IconError::DBusPixmap(zbus::Error::FDO(e))),
            },
            Err(e) => Err(IconError::DBusPixmap(e)),
        }
    }

    pub async fn icon(&self, size: i32) -> std::result::Result<gtk::gdk_pixbuf::Pixbuf, IconError> {
        // TODO make this function retun just Pixbuf instead of a result, now that we're handling
        // all errors here?

        // "Visualizations are encouraged to prefer icon names over icon pixmaps if both are
        // available."

        match self.icon_from_name(size).await {
            Ok(pb) => return Ok(pb),
            Err(IconError::NotAvailable)
            | Err(IconError::LoadIconFromTheme { .. })
                => {},
            // Don't fail icon loading here -- e.g. discord raises
            // "org.freedesktop.DBus.Error.Failed: error occurred in Get" but has a valid pixmap
            Err(e) => log::warn!("failed to get icon by name for {}: {}", self.sni.destination(), e),
        }

        match self.icon_from_pixmap(size).await {
            Ok(pb) => return Ok(pb),
            Err(IconError::NotAvailable) => {},
            Err(e) => log::warn!("failed to get icon pixmap for {}: {}", self.sni.destination(), e),
        }

        fallback_icon(size).await
    }
}
