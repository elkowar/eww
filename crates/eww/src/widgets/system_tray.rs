use gtk::{
    prelude::{ContainerExt, IconThemeExt, WidgetExt},
    traits::{GtkMenuItemExt, MenuShellExt},
    IconLookupFlags, Menu, MenuBar, MenuItem, Orientation, SeparatorMenuItem,
};
use once_cell::sync::Lazy;
use std::{collections::HashMap, sync::Mutex, thread};
use stray::{
    message::{
        menu::{MenuType, TrayMenu},
        tray::StatusNotifierItem,
        NotifierItemCommand, NotifierItemMessage,
    },
    tokio_stream::StreamExt,
    SystemTray,
};
use tokio::{runtime::Runtime, sync::mpsc};

struct NotifierItem {
    item: StatusNotifierItem,
    menu: Option<TrayMenu>,
}

pub struct StatusNotifierWrapper {
    menu: stray::message::menu::MenuItem,
}

static STATE: Lazy<Mutex<HashMap<String, NotifierItem>>> = Lazy::new(|| Mutex::new(HashMap::new()));

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
        self.item.icon_theme_path.as_ref().map(|path| {
            let theme = gtk::IconTheme::new();
            theme.append_search_path(&path);
            let icon_name = self.item.icon_name.as_ref().unwrap();
            let icon_info = theme.lookup_icon(icon_name, 24, IconLookupFlags::empty()).expect("Failed to lookup icon info");

            gtk::Image::from_pixbuf(icon_info.load_icon().ok().as_ref())
        })
    }
}

pub fn start_communication_thread(sender: mpsc::Sender<NotifierItemMessage>, cmd_rx: mpsc::Receiver<NotifierItemCommand>) {
    thread::spawn(move || {
        let runtime = Runtime::new().expect("Failed to create tokio RT");

        runtime.block_on(async {
            let mut tray = SystemTray::new(cmd_rx).await;

            while let Some(message) = tray.next().await {
                sender.send(message).await.expect("failed to send message to UI");
            }
        })
    });
}

pub fn spawn_local_handler(
    v_box: MenuBar,
    mut receiver: mpsc::Receiver<NotifierItemMessage>,
    cmd_tx: mpsc::Sender<NotifierItemCommand>,
) {
    let main_context = glib::MainContext::default();
    let future = async move {
        while let Some(item) = receiver.recv().await {
            let mut state = STATE.lock().unwrap();

            match item {
                NotifierItemMessage::Update { address: id, item, menu } => {
                    state.insert(id, NotifierItem { item, menu });
                }
                NotifierItemMessage::Remove { address } => {
                    state.remove(&address);
                }
            }

            for child in v_box.children() {
                v_box.remove(&child);
            }

            for (address, notifier_item) in state.iter() {
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
                                item.to_menu_item(cmd_tx.clone(), address, menu_path)
                            })
                            .for_each(|item| menu.append(&item));

                        if !tray_menu.submenus.is_empty() {
                            menu_item.set_submenu(Some(&menu));
                        }
                    }
                    v_box.append(&menu_item);
                };

                v_box.show_all();
            }
        }
    };

    main_context.spawn_local(future);
}
