use crate::*;

use gtk::{self, prelude::*};

#[derive(thiserror::Error, Debug)]
enum IconError {
    #[error("while fetching icon name: {0}")]
    DBusIconName(#[source] zbus::Error),
    #[error("while fetching icon theme path: {0}")]
    DBusTheme(#[source] zbus::Error),
    #[error("while fetching pixmap: {0}")]
    DBusPixmap(#[source] zbus::Error),
    #[error("loading icon from file {path:?}")]
    LoadIconFromFile {
        path: String,
        #[source]
        source: gtk::glib::Error,
    },
    #[error("loading icon {icon_name:?} from theme {}", .theme_path.as_ref().unwrap_or(&"(default)".to_owned()))]
    LoadIconFromTheme {
        icon_name: String,
        theme_path: Option<String>,
        #[source]
        source: gtk::glib::Error,
    },
    #[error("no icon available")]
    NotAvailable,
}

/// Get the fallback GTK icon, as a final fallback if the tray item has no icon.
async fn fallback_icon(size: i32, scale: i32) -> Option<gtk::gdk_pixbuf::Pixbuf> {
    let theme = gtk::IconTheme::default().expect("Could not get default gtk theme");
    match theme.load_icon_for_scale("image-missing", size, scale, gtk::IconLookupFlags::FORCE_SIZE) {
        Ok(pb) => pb,
        Err(e) => {
            log::error!("failed to load \"image-missing\" from default theme: {}", e);
            None
        }
    }
}

/// Load a pixbuf from StatusNotifierItem's [Icon format].
///
/// [Icon format]: https://freedesktop.org/wiki/Specifications/StatusNotifierItem/Icons/
fn icon_from_pixmap(width: i32, height: i32, mut data: Vec<u8>) -> gtk::gdk_pixbuf::Pixbuf {
    // We need to convert data from ARGB32 to RGBA32, since that's the only one that gdk-pixbuf
    // understands.
    for chunk in data.chunks_exact_mut(4) {
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

/// From a list of pixmaps, create an icon from the most appropriately sized one.
///
/// This function returns None if and only if no pixmaps are provided.
fn icon_from_pixmaps(pixmaps: Vec<(i32, i32, Vec<u8>)>, size: i32) -> Option<gtk::gdk_pixbuf::Pixbuf> {
    pixmaps
        .into_iter()
        .max_by(|(w1, h1, _), (w2, h2, _)| {
            // take smallest one bigger than requested size, otherwise take biggest
            let a = size * size;
            let a1 = w1 * h1;
            let a2 = w2 * h2;
            match (a1 >= a, a2 >= a) {
                (true, true) => a2.cmp(&a1),
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                (false, false) => a1.cmp(&a2),
            }
        })
        .and_then(|(w, h, d)| {
            let pixbuf = icon_from_pixmap(w, h, d);
            if w != size || h != size {
                pixbuf.scale_simple(size, size, gtk::gdk_pixbuf::InterpType::Bilinear)
            } else {
                Some(pixbuf)
            }
        })
}

/// Load an icon with a given name from either the default (if `theme_path` is `None`), or from the
/// theme at a path.
fn icon_from_name(
    icon_name: &str,
    theme_path: Option<&str>,
    size: i32,
    scale: i32,
) -> std::result::Result<gtk::gdk_pixbuf::Pixbuf, IconError> {
    let theme = if let Some(path) = theme_path {
        let theme = gtk::IconTheme::new();
        theme.prepend_search_path(path);
        theme
    } else {
        gtk::IconTheme::default().expect("Could not get default gtk theme")
    };

    match theme.load_icon_for_scale(icon_name, size, scale, gtk::IconLookupFlags::FORCE_SIZE) {
        Ok(pb) => Ok(pb.expect("no pixbuf from theme.load_icon despite no error")),
        Err(e) => Err(IconError::LoadIconFromTheme {
            icon_name: icon_name.to_owned(),
            theme_path: theme_path.map(str::to_owned),
            source: e,
        }),
    }
}

pub async fn load_icon_from_sni(
    sni: &proxy::StatusNotifierItemProxy<'_>,
    size: i32,
    scale: i32,
) -> Option<gtk::gdk_pixbuf::Pixbuf> {
    // "Visualizations are encouraged to prefer icon names over icon pixmaps if both are
    // available."

    let scaled_size = size * scale;

    // First, see if we can get an icon from the name they provide, using either the theme they
    // specify or the default.
    let icon_from_name: std::result::Result<gtk::gdk_pixbuf::Pixbuf, IconError> = (async {
        // fetch icon name
        let icon_name = sni.icon_name().await;
        log::debug!("dbus: {} icon_name -> {:?}", sni.inner().destination(), icon_name);
        let icon_name = match icon_name {
            Ok(s) if s.is_empty() => return Err(IconError::NotAvailable),
            Ok(s) => s,
            Err(e) => return Err(IconError::DBusIconName(e)),
        };

        // interpret it as an absolute path if we can
        let icon_path = std::path::Path::new(&icon_name);
        if icon_path.is_absolute() && icon_path.is_file() {
            return gtk::gdk_pixbuf::Pixbuf::from_file_at_size(icon_path, scaled_size, scaled_size)
                .map_err(|e| IconError::LoadIconFromFile { path: icon_name, source: e });
        }

        // otherwise, fetch icon theme and lookup using icon_from_name
        let icon_theme_path = sni.icon_theme_path().await;
        log::debug!("dbus: {} icon_theme_path -> {:?}", sni.inner().destination(), icon_theme_path);
        let icon_theme_path = match icon_theme_path {
            Ok(p) if p.is_empty() => None,
            Ok(p) => Some(p),
            // treat property not existing as the same as it being empty i.e. to use the default
            // system theme
            Err(zbus::Error::FDO(e)) => match *e {
                zbus::fdo::Error::UnknownProperty(_) | zbus::fdo::Error::InvalidArgs(_) => None,
                // this error is reported by discord, blueman-applet
                zbus::fdo::Error::Failed(msg) if msg == "error occurred in Get" => None,
                _ => return Err(IconError::DBusTheme(zbus::Error::FDO(e))),
            },
            Err(e) => return Err(IconError::DBusTheme(e)),
        };

        let icon_theme_path: Option<&str> = match &icon_theme_path {
            // this looks weird but this converts &String to &str
            Some(s) => Some(s),
            None => None,
        };
        icon_from_name(&icon_name, icon_theme_path, size, scale)
    })
    .await;

    match icon_from_name {
        Ok(p) => return Some(p),           // got an icon!
        Err(IconError::NotAvailable) => {} // this error is expected, don't log
        Err(e) => log::warn!("failed to get icon by name for {}: {}", sni.inner().destination(), e),
    };

    // Can't get it from name + theme, try the pixmap
    let icon_from_pixmaps = match sni.icon_pixmap().await {
        Ok(ps) => match icon_from_pixmaps(ps, scaled_size) {
            Some(p) => Ok(p),
            None => Err(IconError::NotAvailable),
        },
        Err(zbus::Error::FDO(e)) => match *e {
            // property not existing is an expected error
            zbus::fdo::Error::UnknownProperty(_) | zbus::fdo::Error::InvalidArgs(_) => Err(IconError::NotAvailable),

            _ => Err(IconError::DBusPixmap(zbus::Error::FDO(e))),
        },
        Err(e) => Err(IconError::DBusPixmap(e)),
    };
    match icon_from_pixmaps {
        Ok(p) => return Some(p),
        Err(IconError::NotAvailable) => {}
        Err(e) => log::warn!("failed to get icon pixmap for {}: {}", sni.inner().destination(), e),
    };

    // Tray didn't provide a valid icon so use the default fallback one.
    fallback_icon(size, scale).await
}
