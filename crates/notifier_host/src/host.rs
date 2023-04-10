use crate::*;

use zbus::export::ordered_stream::{self, OrderedStreamExt};

pub trait Host {
    fn add_item(&mut self, id: &str, item: Item);
    fn remove_item(&mut self, id: &str);
}

/// Add a new well-known name of format `org.freedesktop.StatusNotifierHost-{pid}-{nr}` for this connection.
pub async fn attach_new_wellknown_name(con: &zbus::Connection) -> zbus::Result<zbus::names::WellKnownName<'static>> {
    let pid = std::process::id();
    let mut i = 0;
    let wellknown = loop {
        use zbus::fdo::RequestNameReply::*;

        i += 1;
        let wellknown = format!("org.freedesktop.StatusNotifierHost-{}-{}", pid, i);
        let wellknown: zbus::names::WellKnownName = wellknown.try_into().expect("generated well-known name is invalid");

        let flags = [zbus::fdo::RequestNameFlags::DoNotQueue];
        match con.request_name_with_flags(&wellknown, flags.into_iter().collect()).await? {
            PrimaryOwner => break wellknown,
            Exists => {},
            AlreadyOwner => {},
            InQueue => unreachable!("request_name_with_flags returned InQueue even though we specified DoNotQueue"),
        };
    };
    Ok(wellknown)
}

pub async fn run_host_forever(host: &mut dyn Host, con: &zbus::Connection, name: &zbus::names::WellKnownName<'_>) -> zbus::Result<()> {
    // register ourself to StatusNotifierWatcher
    let snw = dbus::StatusNotifierWatcherProxy::new(&con).await?;
    snw.register_status_notifier_host(&name).await?;

    enum ItemEvent {
        NewItem(dbus::StatusNotifierItemRegistered),
        GoneItem(dbus::StatusNotifierItemUnregistered),
    }

    // start listening to these streams
    let new_items = snw.receive_status_notifier_item_registered().await?;
    let gone_items = snw.receive_status_notifier_item_unregistered().await?;

    let mut item_names = std::collections::HashSet::new();

    // initial items first
    for svc in snw.registered_status_notifier_items().await? {
        match Item::from_address(snw.connection(), &svc).await {
            Ok(item) => {
                item_names.insert(svc.to_owned());
                host.add_item(&svc, item);
            },
            Err(e) => {
                log::warn!("Could not create StatusNotifierItem from address {:?}: {:?}", svc, e);
            },
        }
    }

    let mut ev_stream = ordered_stream::join(
        OrderedStreamExt::map(new_items, ItemEvent::NewItem),
        OrderedStreamExt::map(gone_items, ItemEvent::GoneItem),
    );
    while let Some(ev) = ev_stream.next().await {
        match ev {
            ItemEvent::NewItem(sig) => {
                let svc = sig.args()?.service;
                if item_names.contains(svc) {
                    log::info!("Got duplicate new item: {:?}", svc);
                } else {
                    match Item::from_address(snw.connection(), svc).await {
                        Ok(item) => {
                            item_names.insert(svc.to_owned());
                            host.add_item(svc, item);
                        },
                        Err(e) => {
                            log::warn!("Could not create StatusNotifierItem from address {:?}: {:?}", svc, e);
                        },
                    }
                }
            },
            ItemEvent::GoneItem(sig) => {
                let svc = sig.args()?.service;
                if item_names.remove(svc) {
                    host.remove_item(svc);
                }
            },
        }
    }

    // TODO handle running out of events? why could this happen?

    Ok(())
}
