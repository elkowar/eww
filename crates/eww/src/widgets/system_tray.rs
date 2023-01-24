use gtk::{
    prelude::{ContainerExt, IconThemeExt, WidgetExt},
    traits::{GtkMenuItemExt, MenuShellExt},
    IconLookupFlags, Menu, MenuBar, MenuItem, Orientation, SeparatorMenuItem,
};
use once_cell::sync::Lazy;
use std::{collections::HashMap, thread};
use stray::{
    message::{
        menu::{MenuType, TrayMenu},
        tray::{StatusNotifierItem, Status},
        NotifierItemCommand, NotifierItemMessage,
    },
    StatusNotifierWatcher,
};
use tokio::{runtime::Runtime, sync::mpsc};

use crate::loop_select;

#[derive(Clone, Debug)]
struct NotifierItem {
    item: StatusNotifierItem,
    menu: Option<TrayMenu>,
}

// FIXME dropping and whatnot
struct NotifierService {
    state: tokio::sync::watch::Receiver<HashMap<String, NotifierItem>>,
    cmd_tx: mpsc::Sender<NotifierItemCommand>,
    // exit: tokio::sync::oneshot::Sender<()>,
}

// FIXME only run this while we have a status bar
static NOTIFIER_SERVICE: Lazy<NotifierService> = Lazy::new(|| NotifierService::spawn_new());

impl NotifierService {
    fn spawn_new() -> NotifierService {
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        let (watch_tx, watch_rx) = tokio::sync::watch::channel(HashMap::new());
        // let (exit_tx, mut exit_rx) = tokio::sync::oneshot::channel();

        thread::spawn(move || {
            log::info!("Starting shared notifier tray service");
            let mut items = HashMap::<String, NotifierItem>::new();

            let runtime = Runtime::new().expect("Failed to create tokio runtime");
            runtime.block_on(async {
                let snw = StatusNotifierWatcher::new(cmd_rx).await.unwrap();
                let mut host = snw.create_notifier_host("eww").await.unwrap();

                loop_select! {
                    msg = host.recv() => {
                        match msg.unwrap() {
                            NotifierItemMessage::Update { address, item, menu } => {
                                log::info!(
                                    "tray: Update id={:?} status={:?}",
                                    item.id,
                                    item.status,
                                );
                                items.insert(address, NotifierItem { item: *item, menu });
                            },
                            NotifierItemMessage::Remove { address } => {
                                log::info!("tray: Remove id={:?}", items.get(&address).map(|i| i.item.id.clone()));
                                items.remove(&address);
                            },
                        };
                        watch_tx.send(items.clone()).unwrap();
                    },
                    // _ = &mut exit_rx => break,
                }

                // host.destroy().await.unwrap();
            })
        });

        NotifierService {
            state: watch_rx,
            cmd_tx,
            // exit: exit_tx,
        }
    }
}

pub struct StatusNotifierWrapper {
    menu: stray::message::menu::MenuItem,
}

impl StatusNotifierWrapper {
    fn to_menu_item(self, sender: mpsc::Sender<NotifierItemCommand>, notifier_address: String, menu_path: String) -> MenuItem {
        let item: Box<dyn AsRef<MenuItem>> = match self.menu.menu_type {
            MenuType::Separator => Box::new(SeparatorMenuItem::new()),
            MenuType::Standard => Box::new(MenuItem::with_label(self.menu.label.as_str())),
        };

        let item = (*item).as_ref().clone();

        {
            let sender = sender.clone();
            let notifier_address = notifier_address.clone();
            let menu_path = menu_path.clone();

            item.connect_activate(move |_item| {
                sender
                    .try_send(NotifierItemCommand::MenuItemClicked {
                        submenu_id: self.menu.id,
                        menu_path: menu_path.clone(),
                        notifier_address: notifier_address.clone(),
                    })
                    .unwrap();
            });
        };

        let submenu = Menu::new();
        if !self.menu.submenu.is_empty() {
            for submenu_item in self.menu.submenu.iter().cloned() {
                let submenu_item = StatusNotifierWrapper { menu: submenu_item };
                let submenu_item = submenu_item.to_menu_item(sender.clone(), notifier_address.clone(), menu_path.clone());
                submenu.append(&submenu_item);
            }

            item.set_submenu(Some(&submenu));
        }

        item
    }
}

impl NotifierItem {
    fn get_icon(&self) -> Option<gtk::Image> {
        let icon_name = self.item.icon_name.as_ref().unwrap();

        if let Some(path) = self.item.icon_theme_path.as_ref() && !path.is_empty() {
            // custom icon path specified, look there
            let theme = gtk::IconTheme::new();
            theme.prepend_search_path(path);

            match theme.load_icon(icon_name, 24, IconLookupFlags::FORCE_SIZE) {
                Err(e) => log::warn!("Could not find icon {:?} in path {:?}: {}", path, theme, e),
                Ok(pb) => return Some(gtk::Image::from_pixbuf(pb.as_ref())),
            }
        }

        // try default theme
        let theme = gtk::IconTheme::default().expect("Could not get default gtk theme");
        match theme.load_icon(icon_name, 24, IconLookupFlags::FORCE_SIZE) {
            Err(e) => log::warn!("Could not find icon {:?} in default theme: {}", icon_name, e),
            Ok(pb) => return Some(gtk::Image::from_pixbuf(pb.as_ref())),
        }

        // still no icon, use fallback image
        match theme.load_icon("image-missing", 24, IconLookupFlags::FORCE_SIZE) {
            Err(e) => log::error!("Could not find fallback icon \"image-missing\" in default theme: {}", e),
            Ok(pb) => return Some(gtk::Image::from_pixbuf(pb.as_ref())),
        }

        None
    }
}

pub fn maintain_menubar(vbox: MenuBar) {
    let fut = async move {
        let mut rx = NOTIFIER_SERVICE.state.clone();
        while let Ok(()) = rx.changed().await {
            // FIXME don't recreate all icons on update, so menus don't get destroyed
            let items = rx.borrow();

            for child in vbox.children() {
                vbox.remove(&child);
            }

            for (address, notifier_item) in items.iter() {
                // TODO bug in stray: they're parsed the wrong way around
                if let Status::Active = notifier_item.item.status {
                    // FIXME make this behaviour customisable
                    continue // don't display; see documentation of Status
                }

                if let Some(icon) = notifier_item.get_icon() {
                    // Create the menu

                    let menu_item = MenuItem::new();
                    let menu_item_box = gtk::Box::new(Orientation::Horizontal, 3);
                    menu_item_box.add(&icon);
                    menu_item.add(&menu_item_box);

                    if let Some(tray_menu) = &notifier_item.menu {
                        let menu = Menu::new();
                        tray_menu
                            .submenus
                            .iter()
                            .map(|submenu| StatusNotifierWrapper { menu: submenu.to_owned() })
                            .map(|item| {
                                let menu_path = notifier_item.item.menu.as_ref().unwrap().to_string();
                                let address = address.to_string();
                                item.to_menu_item(NOTIFIER_SERVICE.cmd_tx.clone(), address, menu_path)
                            })
                            .for_each(|item| menu.append(&item));

                        if !tray_menu.submenus.is_empty() {
                            menu_item.set_submenu(Some(&menu));
                        }
                    }
                    vbox.append(&menu_item);
                };

                vbox.show_all();
            }
        }
    };

    glib::MainContext::default().spawn_local(fut);
}
