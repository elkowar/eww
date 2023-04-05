#![allow(unused)]

use gtk::prelude::*;
use notifier_host;

async fn gtk_run<F, R1, R>(f: F) -> R
where
    F: FnOnce() -> R1 + 'static,
    R1: std::future::Future<Output=R>,
    R: 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    glib::MainContext::default().spawn_local(async move {
        let r = f().await;
        tx.send(r).map_err(|_| ()).unwrap();
    });
    rx.await.unwrap()
}

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

pub fn maintain_menubar(menubar: gtk::MenuBar) {
    menubar.show_all();
    glib::MainContext::default().spawn_local(async move {
        let mut host = Host {
            menubar,
            items: std::collections::HashMap::new(),
        };
        notifier_host::serve(&mut host, "eww").await.unwrap();
    });
}
