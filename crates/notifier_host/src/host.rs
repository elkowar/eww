use crate::*;

use zbus::export::ordered_stream::{self, OrderedStreamExt};

/// Trait for system tray implementations, to be notified of changes to what items are in the tray.
pub trait Host {
    /// Called when an item is added to the tray. This is also called for all existing items when
    /// starting [`run_host`].
    fn add_item(&mut self, id: &str, item: Item);

    /// Called when an item is removed from the tray.
    fn remove_item(&mut self, id: &str);
}

// TODO We aren't really thinking about what happens when we shut down a host. Currently, we don't
// provide a way to unregister as a host.
//
// It would also be good to combine `register_as_host` and `run_host`, so that we're only
// registered while we're running.

/// Register this DBus connection as a StatusNotifierHost (i.e. system tray).
///
/// This associates with the DBus connection new name of the format
/// `org.freedesktop.StatusNotifierHost-{pid}-{nr}`, and registers it to active
/// StatusNotifierWatcher. The name and the StatusNotifierWatcher proxy are returned.
///
/// You still need to call [`run_host`] to have the instance of [`Host`] be notified of new and
/// removed items.
pub async fn register_as_host(
    con: &zbus::Connection,
) -> zbus::Result<(zbus::names::WellKnownName<'static>, proxy::StatusNotifierWatcherProxy<'static>)> {
    let snw = proxy::StatusNotifierWatcherProxy::new(con).await?;

    // get a well-known name
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
            Exists => {}
            AlreadyOwner => {}
            InQueue => unreachable!("request_name_with_flags returned InQueue even though we specified DoNotQueue"),
        };
    };

    // register it to the StatusNotifierWatcher, so that they know there is a systray on the system
    snw.register_status_notifier_host(&wellknown).await?;

    Ok((wellknown, snw))
}

/// Run the Host forever, calling its methods as signals are received from the StatusNotifierWatcher.
///
/// Before calling this, you should have called [`register_as_host`] (which returns an instance of
/// [`proxy::StatusNotifierWatcherProxy`]).
///
/// This async function runs forever, and only returns if it gets an error! As such, it is
/// recommended to call this via something like `tokio::spawn` that runs this in the
/// background.
pub async fn run_host(host: &mut dyn Host, snw: &proxy::StatusNotifierWatcherProxy<'static>) -> zbus::Error {
    // Replacement for ? operator since we're not returning a Result.
    macro_rules! try_ {
        ($e:expr) => {
            match $e {
                Ok(x) => x,
                Err(e) => return e,
            }
        };
    }

    enum ItemEvent {
        NewItem(proxy::StatusNotifierItemRegistered),
        GoneItem(proxy::StatusNotifierItemUnregistered),
    }

    // start listening to these streams
    let new_items = try_!(snw.receive_status_notifier_item_registered().await);
    let gone_items = try_!(snw.receive_status_notifier_item_unregistered().await);

    let mut item_names = std::collections::HashSet::new();

    // initial items first
    for svc in try_!(snw.registered_status_notifier_items().await) {
        match Item::from_address(snw.inner().connection(), &svc).await {
            Ok(item) => {
                item_names.insert(svc.to_owned());
                host.add_item(&svc, item);
            }
            Err(e) => {
                log::warn!("Could not create StatusNotifierItem from address {:?}: {:?}", svc, e);
            }
        }
    }

    let mut ev_stream = ordered_stream::join(
        OrderedStreamExt::map(new_items, ItemEvent::NewItem),
        OrderedStreamExt::map(gone_items, ItemEvent::GoneItem),
    );
    while let Some(ev) = ev_stream.next().await {
        match ev {
            ItemEvent::NewItem(sig) => {
                let svc = try_!(sig.args()).service;
                if item_names.contains(svc) {
                    log::info!("Got duplicate new item: {:?}", svc);
                } else {
                    match Item::from_address(snw.inner().connection(), svc).await {
                        Ok(item) => {
                            item_names.insert(svc.to_owned());
                            host.add_item(svc, item);
                        }
                        Err(e) => {
                            log::warn!("Could not create StatusNotifierItem from address {:?}: {:?}", svc, e);
                        }
                    }
                }
            }
            ItemEvent::GoneItem(sig) => {
                let svc = try_!(sig.args()).service;
                if item_names.remove(svc) {
                    host.remove_item(svc);
                }
            }
        }
    }

    // I do not know whether this is possible to reach or not.
    unreachable!("StatusNotifierWatcher stopped producing events")
}
