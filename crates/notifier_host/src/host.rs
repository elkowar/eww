use crate::*;

use zbus::export::ordered_stream::{self, OrderedStreamExt};

/// Trait for system tray implementations, to be notified of changes to what items are in the tray.
pub trait Host {
    /// Called when an item is added to the tray. This is also called for all existing items when
    /// starting [`run_host`].
    fn add_item(&mut self, id: &str, item: Item);

    /// Called when an item is removed from the tray.
    fn remove_item(&mut self, id: &str);

    /// Called by [`run_host_forever`] before re-bootstrapping against a new StatusNotifierWatcher
    /// owner. Implementations should remove all currently-displayed items; they will be
    /// re-enumerated against the new owner.
    fn clear(&mut self);
}

// The StatusNotifier spec defines no method to unregister a host; a host's lifetime is tracked
// implicitly by the watcher watching its well-known name, so the name is held for the lifetime of
// the connection and released only when the connection drops. Accordingly we acquire a single
// stable host name per connection and reuse it across re-registrations (see `register_as_host`).
//
// It would also be good to combine `register_as_host` and `run_host`, so that we're only
// registered while we're running.

/// Register this DBus connection as a StatusNotifierHost (i.e. system tray).
///
/// This associates the DBus connection with a name of the format
/// `org.freedesktop.StatusNotifierHost-{pid}-{nr}`, and registers it to the active
/// StatusNotifierWatcher. The name and the StatusNotifierWatcher proxy are returned.
///
/// If the connection already owns such a host name (e.g. from a previous call), that name is
/// reused rather than a fresh one being claimed, so repeated registrations on one connection do
/// not accumulate host names on the bus.
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
            // Reuse a name we already hold instead of claiming a fresh one, so repeated
            // registrations on this connection don't accumulate host names on the bus.
            PrimaryOwner | AlreadyOwner => break wellknown,
            Exists => {} // owned by another connection; try the next index
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
        match Item::from_address(snw.connection(), &svc).await {
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
                    match Item::from_address(snw.connection(), svc).await {
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

/// Run a Host indefinitely, re-bootstrapping host registration and signal subscriptions whenever
/// the `org.kde.StatusNotifierWatcher` well-known name changes ownership.
///
/// This is the recommended entry point for hosts that may start before the system's
/// StatusNotifierWatcher is fully owned by a single stable process (e.g. when a transient
/// fallback watcher like libayatana-appindicator's claims the name during graphical session
/// startup, then exits when a longer-lived watcher takes over).
pub async fn run_host_forever(host: &mut dyn Host, con: &zbus::Connection) -> zbus::Error {
    macro_rules! try_ {
        ($e:expr) => {
            match $e {
                Ok(x) => x,
                Err(e) => return e.into(),
            }
        };
    }

    let dbus = try_!(zbus::fdo::DBusProxy::new(con).await);
    let watcher_bus = zbus::names::BusName::try_from(crate::names::WATCHER_BUS)
        .expect("WATCHER_BUS is a valid well-known name");

    loop {
        let mut owner_changes = try_!(dbus.receive_name_owner_changed_with_args(&[(0, &watcher_bus)]).await);

        // Block until the watcher name has an owner so `register_as_host` has something to call.
        // Handles both the initial-startup case (no watcher yet) and a fast owner-flap where the
        // previous owner has departed but no replacement has claimed the name yet.
        let initial_owner = loop {
            match dbus.get_name_owner(watcher_bus.as_ref()).await {
                Ok(name) => break name.to_string(),
                Err(zbus::fdo::Error::NameHasNoOwner(_)) => {
                    while let Some(sig) = owner_changes.next().await {
                        let args = match sig.args() {
                            Ok(a) => a,
                            Err(_) => continue,
                        };
                        if args.new_owner().is_some() {
                            break;
                        }
                    }
                }
                Err(e) => return e.into(),
            }
        };

        log::debug!("clearing tray items before (re-)bootstrap against watcher owner {}", initial_owner);
        host.clear();

        let (_, snw) = try_!(register_as_host(con).await);

        let owner_changed = async {
            while let Some(sig) = owner_changes.next().await {
                let args = match sig.args() {
                    Ok(a) => a,
                    Err(_) => continue,
                };
                let new_owner = args.new_owner().as_ref().map(|n| n.to_string());

                if new_owner.as_deref() != Some(initial_owner.as_str()) {
                    return;
                }
            }
        };

        // Exiting the select drops `snw` and its signal subscriptions, so the next iteration
        // rebinds against whoever owns the watcher name then.
        tokio::select! {
            err = run_host(host, &snw) => return err,
            _ = owner_changed => {
                log::info!(
                    "StatusNotifierWatcher ownership changed (was {}); re-bootstrapping host",
                    initial_owner
                );
            }
        }
    }
}
