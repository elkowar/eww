#![allow(unused)]

use gtk::prelude::*;
use notifier_host;

struct Host {
    menubar: gtk::MenuBar,
    items: std::collections::HashMap<String, gtk::MenuItem>,
}

async fn watch_foreach<T: std::fmt::Debug>(mut rx: tokio::sync::watch::Receiver<T>, mut f: impl FnMut(&T)) {
    f(&rx.borrow());
    while rx.changed().await.is_ok() {
        f(&rx.borrow());
    }
}

impl notifier_host::Host for Host {
    fn add_item(&mut self, id: &str, item: notifier_host::Item) {
        let mi = gtk::MenuItem::new();
        self.menubar.add(&mi);
        if let Some(old_mi) = self.items.insert(id.to_string(), mi.clone()) {
            self.menubar.remove(&old_mi);
        }

        // maintain title
        glib::MainContext::default().spawn_local({
            let mi = mi.clone();
            watch_foreach(item.title(), move |title| {
                mi.set_tooltip_text(Some(title));
            })
        });

        let icon = gtk::Image::new();
        mi.add(&icon);

        // other initialisation
        glib::MainContext::default().spawn_local({
            let mi = mi.clone();
            async move {
                let img = item.icon(24).await.unwrap();
                icon.set_from_pixbuf(Some(&img));

                let menu = item.menu().await.unwrap();
                mi.set_submenu(Some(&menu));
            }
        });
        mi.show_all();
    }
    fn remove_item(&mut self, id: &str) {
        if let Some(mi) = self.items.get(id) {
            self.menubar.remove(mi);
        } else {
            log::warn!("Tried to remove nonexistent item {:?} from systray", id);
        }
    }
}

struct DBusGlobalState {
    con: zbus::Connection,
    name: zbus::names::WellKnownName<'static>,
}

async fn dbus_state() -> std::sync::Arc<DBusGlobalState> {
    use tokio::sync::Mutex;
    use std::sync::{Weak, Arc};
    use once_cell::sync::Lazy;
    static DBUS_STATE: Lazy<Mutex<Weak<DBusGlobalState>>> = Lazy::new(Default::default);

    let mut dbus_state = DBUS_STATE.lock().await;
    if let Some(state) = dbus_state.upgrade() {
        state
    } else {
        // TODO error handling?
        let con = zbus::Connection::session().await.unwrap();
        notifier_host::watcher_on(&con).await.unwrap();

        let name = notifier_host::attach_new_wellknown_name(&con).await.unwrap();

        let arc = Arc::new(DBusGlobalState {
            con,
            name,
        });
        *dbus_state = Arc::downgrade(&arc);

        arc
    }
}

pub fn maintain_menubar(menubar: gtk::MenuBar) {
    menubar.show_all();
    glib::MainContext::default().spawn_local(async move {
        let mut host = Host {
            menubar,
            items: std::collections::HashMap::new(),
        };
        let s = &dbus_state().await;
        notifier_host::run_host_forever(&mut host, &s.con, &s.name).await.unwrap();
    });
}
