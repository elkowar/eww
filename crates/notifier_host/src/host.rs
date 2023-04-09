use crate::*;

use zbus::export::ordered_stream::{self, OrderedStreamExt};

pub trait Host {
    fn add_item(&mut self, id: &str, item: Item);
    fn remove_item(&mut self, id: &str);
}

/// Register this connection as a StatusNotifierHost.
pub async fn register_host(con: &zbus::Connection) -> zbus::Result<dbus::StatusNotifierWatcherProxy> {
    // From <https://www.freedesktop.org/wiki/Specifications/StatusNotifierItem/StatusNotifierHost/>:
    //
    // Instances of this service are registered on the Dbus session bus, under a name on the
    // form org.freedesktop.StatusNotifierHost-id where id is an unique identifier, that keeps
    // the names unique on the bus, such as the process-id of the application or another type
    // of identifier if more that one StatusNotifierHost is registered by the same process.

    // pick a new wellknown_name
    let pid = std::process::id();
    let mut i = 0;
    let wellknown_name = loop {
        let wellknown_name = format!("org.freedesktop.StatusNotifierHost-{}-{}", pid, i);
        let flags = [zbus::fdo::RequestNameFlags::DoNotQueue];

        use zbus::fdo::RequestNameReply::*;
        match con.request_name_with_flags(wellknown_name.as_str(), flags.into_iter().collect()).await? {
            PrimaryOwner => break wellknown_name,
            Exists => {},
            AlreadyOwner => {}, // we choose to not use an existing owner, is this correct?
            InQueue => panic!("request_name_with_flags returned InQueue even though we specified DoNotQueue"),
        };

        i += 1;
    };

    // register ourself to StatusNotifierWatcher
    let snw = dbus::StatusNotifierWatcherProxy::new(&con).await?;
    snw.register_status_notifier_host(&wellknown_name).await?;

    Ok(snw)
}

pub async fn serve_host_forever_on(host: &mut dyn Host, snw: dbus::StatusNotifierWatcherProxy<'_>) -> zbus::Result<()> {
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
                    log::warn!("Got duplicate new item: {:?}", svc);
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
