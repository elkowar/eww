use futures::StreamExt;
use gtk::{cairo::Surface, gdk::ffi::gdk_cairo_surface_create_from_pixbuf, prelude::*};
use notifier_host;

// DBus state shared between systray instances, to avoid creating too many connections etc.
struct DBusSession {
    snw: notifier_host::proxy::StatusNotifierWatcherProxy<'static>,
}

async fn dbus_session() -> zbus::Result<&'static DBusSession> {
    // TODO make DBusSession reference counted so it's dropped when not in use?

    static DBUS_STATE: tokio::sync::OnceCell<DBusSession> = tokio::sync::OnceCell::const_new();
    DBUS_STATE
        .get_or_try_init(|| async {
            let con = zbus::Connection::session().await?;
            notifier_host::Watcher::new().attach_to(&con).await?;

            let (_, snw) = notifier_host::register_as_host(&con).await?;

            Ok(DBusSession { snw })
        })
        .await
}

pub struct Props {
    icon_size_tx: tokio::sync::watch::Sender<i32>,
}

impl Props {
    pub fn new() -> Self {
        let (icon_size_tx, _) = tokio::sync::watch::channel(24);
        Self { icon_size_tx }
    }

    pub fn icon_size(&self, value: i32) {
        let _ = self.icon_size_tx.send_if_modified(|x| {
            if *x == value {
                false
            } else {
                *x = value;
                true
            }
        });
    }
}

struct Tray {
    menubar: gtk::MenuBar,
    items: std::collections::HashMap<String, Item>,

    icon_size: tokio::sync::watch::Receiver<i32>,
}

pub fn spawn_systray(menubar: &gtk::MenuBar, props: &Props) {
    let mut systray = Tray { menubar: menubar.clone(), items: Default::default(), icon_size: props.icon_size_tx.subscribe() };

    let task = glib::MainContext::default().spawn_local(async move {
        let s = match dbus_session().await {
            Ok(x) => x,
            Err(e) => {
                log::error!("could not initialise dbus connection for tray: {}", e);
                return;
            }
        };

        systray.menubar.show();
        let e = notifier_host::run_host(&mut systray, &s.snw).await;
        log::error!("notifier host error: {}", e);
    });

    // stop the task when the widget is dropped
    menubar.connect_destroy(move |_| {
        task.abort();
    });
}

impl notifier_host::Host for Tray {
    fn add_item(&mut self, id: &str, item: notifier_host::Item) {
        let item = Item::new(id.to_owned(), item, self.icon_size.clone());
        self.menubar.add(&item.widget);
        if let Some(old_item) = self.items.insert(id.to_string(), item) {
            self.menubar.remove(&old_item.widget);
        }
    }

    fn remove_item(&mut self, id: &str) {
        if let Some(item) = self.items.get(id) {
            self.menubar.remove(&item.widget);
        } else {
            log::warn!("Tried to remove nonexistent item {:?} from systray", id);
        }
    }
}

/// Item represents a single icon being shown in the system tray.
struct Item {
    /// Main widget representing this tray item.
    widget: gtk::MenuItem,

    /// Async task to stop when this item gets removed.
    task: Option<glib::JoinHandle<()>>,
}

impl Drop for Item {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

impl Item {
    fn new(id: String, item: notifier_host::Item, icon_size: tokio::sync::watch::Receiver<i32>) -> Self {
        let widget = gtk::MenuItem::new();
        let out_widget = widget.clone(); // copy so we can return it

        let task = glib::MainContext::default().spawn_local(async move {
            if let Err(e) = Item::maintain(widget.clone(), item, icon_size).await {
                log::error!("error for systray item {}: {}", id, e);
            }
        });

        Self { widget: out_widget, task: Some(task) }
    }

    async fn maintain(
        widget: gtk::MenuItem,
        item: notifier_host::Item,
        mut icon_size: tokio::sync::watch::Receiver<i32>,
    ) -> zbus::Result<()> {
        // init icon
        let icon = gtk::Image::new();
        widget.add(&icon);
        icon.show();

        // init menu
        match item.menu().await {
            Ok(m) => widget.set_submenu(Some(&m)),
            Err(e) => log::warn!("failed to get menu: {}", e),
        }

        // TODO this is a lot of code duplication unfortunately, i'm not really sure how to
        // refactor without making the borrow checker angry

        // set status
        match item.status().await? {
            notifier_host::Status::Passive => widget.hide(),
            notifier_host::Status::Active | notifier_host::Status::NeedsAttention => widget.show(),
        }

        // set title
        widget.set_tooltip_text(Some(&item.sni.title().await?));

        // set icon
        let scale = icon.scale_factor();
        load_icon_for_item(&icon, &item, *icon_size.borrow_and_update(), scale).await;

        // updates
        let mut status_updates = item.sni.receive_new_status().await?;
        let mut title_updates = item.sni.receive_new_status().await?;
        let mut icon_updates = item.sni.receive_new_icon().await?;

        loop {
            tokio::select! {
                Some(_) = status_updates.next() => {
                    // set status
                    match item.status().await? {
                        notifier_host::Status::Passive => widget.hide(),
                        notifier_host::Status::Active | notifier_host::Status::NeedsAttention => widget.show(),
                    }
                }
                Ok(_) = icon_size.changed() => {
                    // set icon
                    load_icon_for_item(&icon, &item, *icon_size.borrow_and_update(), scale).await;
                }
                Some(_) = title_updates.next() => {
                    // set title
                    widget.set_tooltip_text(Some(&item.sni.title().await?));
                }
                Some(_) = icon_updates.next() => {
                    // set icon
                    load_icon_for_item(&icon, &item, *icon_size.borrow_and_update(), scale).await;
                }
            }
        }
    }
}

async fn load_icon_for_item(icon: &gtk::Image, item: &notifier_host::Item, size: i32, scale: i32) {
    if let Some(pixbuf) = item.icon(size, scale).await {
        let surface = unsafe {
            // gtk::cairo::Surface will destroy the underlying surface on drop
            let ptr = gdk_cairo_surface_create_from_pixbuf(
                pixbuf.as_ptr(),
                scale,
                icon.window().map_or(std::ptr::null_mut(), |v| v.as_ptr()),
            );
            Surface::from_raw_full(ptr)
        };
        icon.set_from_surface(surface.ok().as_ref());
    }
}
