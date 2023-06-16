#![allow(unused)]

use gtk::prelude::*;
use notifier_host::{self, export::ordered_stream::OrderedStreamExt};

// DBus state shared between systray instances, to avoid creating too many connections etc.
struct DBusGlobalState {
    con: zbus::Connection,
    name: zbus::names::WellKnownName<'static>,
}

async fn dbus_state() -> std::sync::Arc<DBusGlobalState> {
    use once_cell::sync::Lazy;
    use std::sync::{Arc, Weak};
    use tokio::sync::Mutex;
    static DBUS_STATE: Lazy<Mutex<Weak<DBusGlobalState>>> = Lazy::new(Default::default);

    let mut dbus_state = DBUS_STATE.lock().await;
    if let Some(state) = dbus_state.upgrade() {
        state
    } else {
        // TODO error handling?
        let con = zbus::Connection::session().await.unwrap();
        notifier_host::Watcher::new().attach_to(&con).await.unwrap();

        let name = notifier_host::attach_new_wellknown_name(&con).await.unwrap();

        let arc = Arc::new(DBusGlobalState { con, name });
        *dbus_state = Arc::downgrade(&arc);

        arc
    }
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

    glib::MainContext::default().spawn_local(async move {
        let s = &dbus_state().await;
        systray.menubar.show();
        notifier_host::run_host_forever(&mut systray, &s.con, &s.name).await.unwrap();
    });
}

impl notifier_host::Host for Tray {
    fn add_item(&mut self, id: &str, item: notifier_host::Item) {
        let item = Item::new(id.to_owned(), item, self.icon_size.clone());
        self.menubar.add(&item.mi);
        if let Some(old_item) = self.items.insert(id.to_string(), item) {
            self.menubar.remove(&old_item.mi);
        }
    }

    fn remove_item(&mut self, id: &str) {
        if let Some(item) = self.items.get(id) {
            self.menubar.remove(&item.mi);
        } else {
            log::warn!("Tried to remove nonexistent item {:?} from systray", id);
        }
    }
}

struct Item {
    mi: gtk::MenuItem,

    tasks: Vec<glib::SourceId>,
}

impl Drop for Item {
    fn drop(&mut self) {
        for task in self.tasks.drain(..) {
            // TODO does this abort the task
            task.remove();
        }
    }
}

impl Item {
    fn new(id: String, item: notifier_host::Item, mut icon_size: tokio::sync::watch::Receiver<i32>) -> Self {
        let mi = gtk::MenuItem::new();
        let mut out = Self { mi: mi.clone(), tasks: Vec::new() };

        out.spawn(async move {
            // TODO don't unwrap so much

            // init icon
            let icon = gtk::Image::new();
            mi.add(&icon);
            icon.show();

            // init menu
            match item.menu().await {
                Ok(m) => mi.set_submenu(Some(&m)),
                Err(e) => log::warn!("failed to get menu of {}: {}", id, e),
            }

            // TODO this is a lot of code duplication unfortunately, i'm not really sure how to
            // refactor without making the borrow checker angry

            // set status
            match item.status().await.unwrap() {
                notifier_host::Status::Passive => mi.hide(),
                notifier_host::Status::Active | notifier_host::Status::NeedsAttention => mi.show(),
            }

            // set title
            mi.set_tooltip_text(Some(&item.sni.title().await.unwrap()));

            // set icon
            icon.set_from_pixbuf(Some(&item.icon(*icon_size.borrow_and_update()).await));

            // updates
            let mut status_updates = item.sni.receive_new_status().await.unwrap();
            let mut title_updates = item.sni.receive_new_status().await.unwrap();

            loop {
                tokio::select! {
                    Some(_) = status_updates.next() => {
                        // set status
                        match item.status().await.unwrap() {
                            notifier_host::Status::Passive => mi.hide(),
                            notifier_host::Status::Active | notifier_host::Status::NeedsAttention => mi.show(),
                        }
                    }
                    Ok(_) = icon_size.changed() => {
                        // set icon
                        icon.set_from_pixbuf(Some(&item.icon(*icon_size.borrow_and_update()).await));
                    }
                    Some(_) = title_updates.next() => {
                        // set title
                        mi.set_tooltip_text(Some(&item.sni.title().await.unwrap()));
                    }
                }
            }
        });

        out
    }

    fn spawn(&mut self, f: impl std::future::Future<Output = ()> + 'static) {
        self.tasks.push(glib::MainContext::default().spawn_local(f));
    }
}
