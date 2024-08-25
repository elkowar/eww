use crate::widgets::window::Window;
use futures::StreamExt;
use gtk::{
    cairo::Surface,
    gdk::{self, ffi::gdk_cairo_surface_create_from_pixbuf, NotifyType},
    glib,
    prelude::*,
};
use std::{cell::RefCell, future::Future, rc::Rc};

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

fn run_async_task<F: Future>(f: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("Failed to initialize tokio runtime");
    rt.block_on(f)
}

pub struct Props {
    icon_size_tx: tokio::sync::watch::Sender<i32>,
    pub prepend_new: Rc<RefCell<bool>>,
}

impl Props {
    pub fn new() -> Self {
        let (icon_size_tx, _) = tokio::sync::watch::channel(24);
        Self { icon_size_tx, prepend_new: Rc::new(RefCell::new(false)) }
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
    container: gtk::Box,
    items: std::collections::HashMap<String, Item>,

    icon_size: tokio::sync::watch::Receiver<i32>,
    prepend_new: Rc<RefCell<bool>>,
}

pub fn spawn_systray(container: &gtk::Box, props: &Props) {
    let mut systray = Tray {
        container: container.clone(),
        items: Default::default(),
        icon_size: props.icon_size_tx.subscribe(),
        prepend_new: props.prepend_new.clone(),
    };

    let task = glib::MainContext::default().spawn_local(async move {
        let s = match dbus_session().await {
            Ok(x) => x,
            Err(e) => {
                log::error!("could not initialise dbus connection for tray: {}", e);
                return;
            }
        };

        systray.container.show();
        let e = notifier_host::run_host(&mut systray, &s.snw).await;
        log::error!("notifier host error: {}", e);
    });

    // stop the task when the widget is dropped
    container.connect_destroy(move |_| {
        task.abort();
    });
}

impl notifier_host::Host for Tray {
    fn add_item(&mut self, id: &str, item: notifier_host::Item) {
        let item = Item::new(id.to_owned(), item, self.icon_size.clone());
        if *self.prepend_new.borrow() {
            self.container.pack_end(&item.widget, true, true, 0);
        } else {
            self.container.pack_start(&item.widget, true, true, 0);
        }
        if let Some(old_item) = self.items.insert(id.to_string(), item) {
            self.container.remove(&old_item.widget);
        }
    }

    fn remove_item(&mut self, id: &str) {
        if let Some(item) = self.items.get(id) {
            self.container.remove(&item.widget);
            self.items.remove(id);
        } else {
            log::warn!("Tried to remove nonexistent item {:?} from systray", id);
        }
    }
}

/// Item represents a single icon being shown in the system tray.
struct Item {
    /// Main widget representing this tray item.
    widget: gtk::EventBox,

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
        let gtk_widget = gtk::EventBox::new();

        // Support :hover selector
        gtk_widget.connect_enter_notify_event(|gtk_widget, evt| {
            if evt.detail() != NotifyType::Inferior {
                gtk_widget.clone().set_state_flags(gtk::StateFlags::PRELIGHT, false);
            }
            glib::Propagation::Proceed
        });

        gtk_widget.connect_leave_notify_event(|gtk_widget, evt| {
            if evt.detail() != NotifyType::Inferior {
                gtk_widget.clone().unset_state_flags(gtk::StateFlags::PRELIGHT);
            }
            glib::Propagation::Proceed
        });

        let out_widget = gtk_widget.clone(); // copy so we can return it

        let task = glib::MainContext::default().spawn_local(async move {
            if let Err(e) = Item::maintain(gtk_widget.clone(), item, icon_size).await {
                log::error!("error for systray item {}: {}", id, e);
            }
        });

        Self { widget: out_widget, task: Some(task) }
    }

    async fn maintain(
        widget: gtk::EventBox,
        mut item: notifier_host::Item,
        mut icon_size: tokio::sync::watch::Receiver<i32>,
    ) -> zbus::Result<()> {
        // init icon
        let icon = gtk::Image::new();
        widget.add(&icon);
        icon.show();

        // init menu
        if let Err(e) = item.set_menu(&widget).await {
            log::warn!("failed to set menu: {}", e);
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

        let item = Rc::new(item);
        let window =
            widget.toplevel().expect("Failed to obtain toplevel window").downcast::<Window>().expect("Failed to downcast window");
        widget.add_events(gdk::EventMask::BUTTON_PRESS_MASK);
        widget.connect_button_press_event(glib::clone!(@strong item => move |_, evt| {
            let (x, y) = (evt.root().0 as i32 + window.x(), evt.root().1 as i32 + window.y());
            let item_is_menu = run_async_task(async { item.sni.item_is_menu().await });
            let have_item_is_menu = item_is_menu.is_ok();
            let item_is_menu = item_is_menu.unwrap_or(false);
            log::debug!(
                "mouse click button={}, x={}, y={}, have_item_is_menu={}, item_is_menu={}",
                evt.button(),
                x,
                y,
                have_item_is_menu,
                item_is_menu
            );

            let result = match (evt.button(), item_is_menu) {
                (gdk::BUTTON_PRIMARY, false) => {
                    let result = run_async_task(async { item.sni.activate(x, y).await });
                    if result.is_err() && !have_item_is_menu {
                        log::debug!("fallback to context menu due to: {}", result.unwrap_err());
                        // Some applications are in fact menu-only (don't have Activate method)
                        // but don't report so through ItemIsMenu property. Fallback to menu if
                        // activate failed in this case.
                        run_async_task(async { item.popup_menu( evt, x, y).await })
                    } else {
                        result
                    }
                }
                (gdk::BUTTON_MIDDLE, _) => run_async_task(async { item.sni.secondary_activate(x, y).await }),
                (gdk::BUTTON_SECONDARY, _) | (gdk::BUTTON_PRIMARY, true) => {
                    run_async_task(async { item.popup_menu( evt, x, y).await })
                }
                _ => Err(zbus::Error::Failure(format!("unknown button {}", evt.button()))),
            };
            if let Err(result) = result {
                log::error!("failed to handle mouse click {}: {}", evt.button(), result);
            }
            glib::Propagation::Stop
        }));

        // updates
        let mut status_updates = item.sni.receive_new_status().await?;
        let mut title_updates = item.sni.receive_new_title().await?;
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
