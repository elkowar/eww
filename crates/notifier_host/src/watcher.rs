use crate::names;
use zbus::{dbus_interface, export::ordered_stream::OrderedStreamExt, Interface};

/// An instance of [`org.kde.StatusNotifierWatcher`]. It only tracks what tray items and trays
/// exist, and doesn't have any logic for displaying items (for that, see [`Host`][`crate::Host`]).
///
/// While this is usually run alongside the tray, it can also be used standalone.
///
/// [`org.kde.StatusNotifierWatcher`]: https://freedesktop.org/wiki/Specifications/StatusNotifierItem/StatusNotifierWatcher/
#[derive(Debug, Default)]
pub struct Watcher {
    // Intentionally using std::sync::Mutex instead of tokio's async mutex, since we don't need to
    // hold the mutex across an await.
    //
    // See <https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html#which-kind-of-mutex-should-you-use>
    tasks: std::sync::Arc<std::sync::Mutex<tokio::task::JoinSet<()>>>,
    hosts: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    items: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
}

/// Implementation of the `StatusNotifierWatcher` service.
///
/// Methods and properties correspond to methods and properties on the DBus service that can be
/// used by others, while signals are events that we generate that other services listen to.
#[dbus_interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    /// RegisterStatusNotifierHost method
    async fn register_status_notifier_host(
        &mut self,
        service: &str,
        #[zbus(header)] hdr: zbus::MessageHeader<'_>,
        #[zbus(connection)] con: &zbus::Connection,
        #[zbus(signal_context)] ctxt: zbus::SignalContext<'_>,
    ) -> zbus::fdo::Result<()> {
        // TODO right now, we convert everything to the unique bus name (something like :1.234).
        // However, it might make more sense to listen to the actual name they give us, so that if
        // the connection dissociates itself from the org.kde.StatusNotifierHost-{pid}-{nr} name
        // but still remains around, we drop them as a host.
        //
        // (This also applies to RegisterStatusNotifierItem)

        let (service, _) = parse_service(service, hdr, con).await?;
        log::info!("new host: {}", service);

        let added_first = {
            // scoped around locking of hosts
            let mut hosts = self.hosts.lock().unwrap(); // unwrap: mutex poisoning is okay
            if !hosts.insert(service.to_string()) {
                // we're already tracking them
                return Ok(());
            }
            hosts.len() == 1
        };

        if added_first {
            self.is_status_notifier_host_registered_changed(&ctxt).await?;
        }
        Watcher::status_notifier_host_registered(&ctxt).await?;

        self.tasks.lock().unwrap().spawn({
            let hosts = self.hosts.clone();
            let ctxt = ctxt.to_owned();
            let con = con.to_owned();
            async move {
                if let Err(e) = wait_for_service_exit(&con, service.as_ref().into()).await {
                    log::error!("failed to wait for service exit: {}", e);
                }
                log::info!("lost host: {}", service);

                let removed_last = {
                    let mut hosts = hosts.lock().unwrap(); // unwrap: mutex poisoning is okay
                    let did_remove = hosts.remove(service.as_str());
                    did_remove && hosts.is_empty()
                };

                if removed_last {
                    if let Err(e) = Watcher::is_status_notifier_host_registered_refresh(&ctxt).await {
                        log::error!("failed to signal Watcher: {}", e);
                    }
                }
                if let Err(e) = Watcher::status_notifier_host_unregistered(&ctxt).await {
                    log::error!("failed to signal Watcher: {}", e);
                }
            }
        });

        Ok(())
    }

    /// StatusNotifierHostRegistered signal.
    #[dbus_interface(signal)]
    async fn status_notifier_host_registered(ctxt: &zbus::SignalContext<'_>) -> zbus::Result<()>;

    /// StatusNotifierHostUnregistered signal
    #[dbus_interface(signal)]
    async fn status_notifier_host_unregistered(ctxt: &zbus::SignalContext<'_>) -> zbus::Result<()>;

    /// IsStatusNotifierHostRegistered property
    #[dbus_interface(property)]
    async fn is_status_notifier_host_registered(&self) -> bool {
        let hosts = self.hosts.lock().unwrap(); // unwrap: mutex poisoning is okay
        !hosts.is_empty()
    }

    // ------------------------------------------------------------------------

    /// RegisterStatusNotifierItem method
    async fn register_status_notifier_item(
        &mut self,
        service: &str,
        #[zbus(header)] hdr: zbus::MessageHeader<'_>,
        #[zbus(connection)] con: &zbus::Connection,
        #[zbus(signal_context)] ctxt: zbus::SignalContext<'_>,
    ) -> zbus::fdo::Result<()> {
        let (service, objpath) = parse_service(service, hdr, con).await?;
        let service = zbus::names::BusName::Unique(service);

        let item = format!("{}{}", service, objpath);

        {
            let mut items = self.items.lock().unwrap(); // unwrap: mutex poisoning is okay
            if !items.insert(item.clone()) {
                // we're already tracking them
                log::info!("new item: {} (duplicate)", item);
                return Ok(());
            }
        }
        log::info!("new item: {}", item);

        self.registered_status_notifier_items_changed(&ctxt).await?;
        Watcher::status_notifier_item_registered(&ctxt, item.as_ref()).await?;

        self.tasks.lock().unwrap().spawn({
            let items = self.items.clone();
            let ctxt = ctxt.to_owned();
            let con = con.to_owned();
            async move {
                if let Err(e) = wait_for_service_exit(&con, service.as_ref()).await {
                    log::error!("failed to wait for service exit: {}", e);
                }
                println!("gone item: {}", &item);

                {
                    let mut items = items.lock().unwrap(); // unwrap: mutex poisoning is okay
                    items.remove(&item);
                }

                if let Err(e) = Watcher::registered_status_notifier_items_refresh(&ctxt).await {
                    log::error!("failed to signal Watcher: {}", e);
                }
                if let Err(e) = Watcher::status_notifier_item_unregistered(&ctxt, item.as_ref()).await {
                    log::error!("failed to signal Watcher: {}", e);
                }
            }
        });

        Ok(())
    }

    /// StatusNotifierItemRegistered signal
    #[dbus_interface(signal)]
    async fn status_notifier_item_registered(ctxt: &zbus::SignalContext<'_>, service: &str) -> zbus::Result<()>;

    /// StatusNotifierItemUnregistered signal
    #[dbus_interface(signal)]
    async fn status_notifier_item_unregistered(ctxt: &zbus::SignalContext<'_>, service: &str) -> zbus::Result<()>;

    /// RegisteredStatusNotifierItems property
    #[dbus_interface(property)]
    async fn registered_status_notifier_items(&self) -> Vec<String> {
        let items = self.items.lock().unwrap(); // unwrap: mutex poisoning is okay
        items.iter().cloned().collect()
    }

    // ------------------------------------------------------------------------

    /// ProtocolVersion property
    #[dbus_interface(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

impl Watcher {
    /// Create a new Watcher.
    pub fn new() -> Watcher {
        Default::default()
    }

    /// Attach and run the Watcher (in the background) on a connection.
    ///
    /// If another process already owns the `org.kde.StatusNotifierWatcher` well-known name, a
    /// background task is spawned that monitors `NameOwnerChanged` and claims the name as soon
    /// as the current owner departs. This avoids the silent state-desync that would otherwise
    /// occur if eww queued for the name and later became primary owner while its host-side
    /// registrations were still bound to the original owner.
    pub async fn attach_to(self, con: &zbus::Connection) -> zbus::Result<()> {
        // Clone before moving `self` into the object server so we can still spawn the claim task
        // onto the Watcher's JoinSet if needed.
        let tasks = self.tasks.clone();

        if !con.object_server().at(names::WATCHER_OBJECT, self).await? {
            return Err(zbus::Error::Failure(format!(
                "Object already exists at {} on this connection -- is StatusNotifierWatcher already running?",
                names::WATCHER_OBJECT
            )));
        }

        // zbus 3.x maps a `RequestNameReply::Exists` response to `Err(NameTaken)`, so both arms
        // mean "another process owns the name."
        let flags = [zbus::fdo::RequestNameFlags::DoNotQueue];
        match con.request_name_with_flags(names::WATCHER_BUS, flags.into_iter().collect()).await {
            Ok(zbus::fdo::RequestNameReply::PrimaryOwner) | Ok(zbus::fdo::RequestNameReply::AlreadyOwner) => Ok(()),
            Err(zbus::Error::NameTaken) => {
                log::info!(
                    "{} is already owned by another process; will claim it when the current owner departs",
                    names::WATCHER_BUS
                );
                let con = con.clone();
                // Self-terminates once the name is acquired. Lives on the Watcher's JoinSet so
                // it's aborted alongside the other background tasks when the Watcher is dropped.
                tasks.lock().unwrap().spawn(async move {
                    if let Err(e) = claim_watcher_name_when_free(&con).await {
                        log::error!("failed to claim {}: {}", names::WATCHER_BUS, e);
                    }
                });

                Ok(())
            }
            Ok(reply) => unreachable!("unexpected RequestName reply with DoNotQueue: {:?}", reply),
            Err(e) => Err(e),
        }
    }

    /// Equivalent to `is_status_notifier_host_registered_invalidate`, but without requiring
    /// `self`.
    async fn is_status_notifier_host_registered_refresh(ctxt: &zbus::SignalContext<'_>) -> zbus::Result<()> {
        zbus::fdo::Properties::properties_changed(
            ctxt,
            Self::name(),
            &std::collections::HashMap::new(),
            &["IsStatusNotifierHostRegistered"],
        )
        .await
    }

    /// Equivalent to `registered_status_notifier_items_invalidate`, but without requiring `self`.
    async fn registered_status_notifier_items_refresh(ctxt: &zbus::SignalContext<'_>) -> zbus::Result<()> {
        zbus::fdo::Properties::properties_changed(
            ctxt,
            Self::name(),
            &std::collections::HashMap::new(),
            &["RegisteredStatusNotifierItems"],
        )
        .await
    }
}

/// Decode the service name that others give to us, into the [bus
/// name](https://dbus2.github.io/zbus/concepts.html#bus-name--service-name) and the [object
/// path](https://dbus2.github.io/zbus/concepts.html#objects-and-object-paths) within the
/// connection.
///
/// The freedesktop.org specification has the format of this be just the bus name, however some
/// status items pass non-conforming values. One common one is just the object path.
async fn parse_service<'a>(
    service: &'a str,
    hdr: zbus::MessageHeader<'_>,
    con: &zbus::Connection,
) -> zbus::fdo::Result<(zbus::names::UniqueName<'static>, &'a str)> {
    if service.starts_with('/') {
        // they sent us just the object path
        if let Some(sender) = hdr.sender()? {
            Ok((sender.to_owned(), service))
        } else {
            log::warn!("unknown sender");
            Err(zbus::fdo::Error::InvalidArgs("Unknown bus address".into()))
        }
    } else {
        // parse the bus name they gave us
        let busname: zbus::names::BusName = match service.try_into() {
            Ok(x) => x,
            Err(e) => {
                log::warn!("received invalid bus name {:?}: {}", service, e);
                return Err(zbus::fdo::Error::InvalidArgs(e.to_string()));
            }
        };

        if let zbus::names::BusName::Unique(unique) = busname {
            Ok((unique.to_owned(), names::ITEM_OBJECT))
        } else {
            // they gave us a "well-known name" like org.kde.StatusNotifierHost-81830-0, we need to
            // convert this into the actual identifier for their bus (e.g. :1.234), so that even if
            // they remove that well-known name it's fine.
            let dbus = zbus::fdo::DBusProxy::new(con).await?;
            match dbus.get_name_owner(busname).await {
                Ok(owner) => Ok((owner.into_inner(), names::ITEM_OBJECT)),
                Err(e) => {
                    log::warn!("failed to get owner of {:?}: {}", service, e);
                    Err(e)
                }
            }
        }
    }
}

/// Wait until the current owner of `org.kde.StatusNotifierWatcher` departs, then claim the name.
///
/// Loops to handle the case where a third party claims the name between when the previous owner
/// departed and when we issued our own `RequestName`.
async fn claim_watcher_name_when_free(con: &zbus::Connection) -> zbus::Result<()> {
    let dbus = zbus::fdo::DBusProxy::new(con).await?;
    let watcher_bus =
        zbus::names::BusName::try_from(names::WATCHER_BUS).expect("WATCHER_BUS is a valid well-known name");
    let mut owner_changes = dbus.receive_name_owner_changed_with_args(&[(0, &watcher_bus)]).await?;

    loop {
        let flags = [zbus::fdo::RequestNameFlags::DoNotQueue];
        match con.request_name_with_flags(names::WATCHER_BUS, flags.into_iter().collect()).await {
            Ok(zbus::fdo::RequestNameReply::PrimaryOwner) | Ok(zbus::fdo::RequestNameReply::AlreadyOwner) => {
                log::info!("acquired {} after previous owner departed", names::WATCHER_BUS);

                return Ok(());
            }
            Err(zbus::Error::NameTaken) => {
                // Someone else still owns it; wait for the next departure and retry.
                while let Some(sig) = owner_changes.next().await {
                    let args = sig.args()?;
                    if args.new_owner().is_none() {
                        break;
                    }
                }
            }
            Ok(reply) => unreachable!("unexpected RequestName reply with DoNotQueue: {:?}", reply),
            Err(e) => return Err(e),
        }
    }
}

/// Wait for a DBus service to disappear
async fn wait_for_service_exit(con: &zbus::Connection, service: zbus::names::BusName<'_>) -> zbus::fdo::Result<()> {
    let dbus = zbus::fdo::DBusProxy::new(con).await?;
    let mut owner_changes = dbus.receive_name_owner_changed_with_args(&[(0, &service)]).await?;

    if !dbus.name_has_owner(service.as_ref()).await? {
        // service has already disappeared
        return Ok(());
    }

    while let Some(sig) = owner_changes.next().await {
        let args = sig.args()?;
        if args.new_owner().is_none() {
            break;
        }
    }

    Ok(())
}
