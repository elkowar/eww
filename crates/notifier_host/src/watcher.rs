use zbus::dbus_interface;
use zbus::Interface;
use zbus::export::ordered_stream::OrderedStreamExt;

pub const WATCHER_BUS_NAME: &'static str = "org.kde.StatusNotifierWatcher";
pub const WATCHER_OBJECT_NAME: &'static str = "/StatusNotifierWatcher";

async fn parse_service<'a>(
    service: &'a str,
    hdr: zbus::MessageHeader<'_>,
    con: &zbus::Connection,
) -> zbus::fdo::Result<(zbus::names::UniqueName<'static>, &'a str)>
{
    if service.starts_with("/") {
        // they sent us just the object path :(
        if let Some(sender) = hdr.sender()? {
            Ok((sender.to_owned(), service))
        } else {
            log::warn!("unknown sender");
            Err(zbus::fdo::Error::InvalidArgs("Unknown bus address".into()))
        }
    } else {
        let busname: zbus::names::BusName = match service.try_into() {
            Ok(x) => x,
            Err(e) => {
                log::warn!("received invalid bus name {:?}: {}", service, e);
                return Err(zbus::fdo::Error::InvalidArgs(e.to_string()));
            },
        };

        if let zbus::names::BusName::Unique(unique) = busname {
            Ok((unique.to_owned(), "/StatusNotifierItem"))
        } else {
            // unwrap: we should always be able to access the dbus interface
            let dbus = zbus::fdo::DBusProxy::new(&con).await.unwrap();
            match dbus.get_name_owner(busname).await {
                Ok(owner) => Ok((owner.into_inner(), "/StatusNotifierItem")),
                Err(e) => {
                    log::warn!("failed to get owner of {:?}: {}", service, e);
                    Err(e)
                }
            }
        }
    }
}

/// Wait for a DBus service to exit
async fn wait_for_service_exit(
    connection: zbus::Connection,
    service: zbus::names::BusName<'_>,
) -> zbus::fdo::Result<()> {
    let dbus = zbus::fdo::DBusProxy::new(&connection).await?;
    let mut owner_changes = dbus
        .receive_name_owner_changed_with_args(&[(0, &service)])
        .await?;

    if !dbus.name_has_owner(service.as_ref()).await? {
        return Ok(())
    }

    while let Some(sig) = owner_changes.next().await {
        let args = sig.args()?;
        if args.new_owner().is_none() {
            break
        }
    }

    Ok(())
}

#[derive(Debug, Default)]
pub struct Watcher {
    tasks: tokio::task::JoinSet<()>,
    hosts: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    items: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
}

#[dbus_interface(name="org.kde.StatusNotifierWatcher")]
impl Watcher {
    /// RegisterStatusNotifierHost method
    async fn register_status_notifier_host(
        &mut self,
        service: &str,
        #[zbus(header)] hdr: zbus::MessageHeader<'_>,
        #[zbus(connection)] con: &zbus::Connection,
        #[zbus(signal_context)] ctxt: zbus::SignalContext<'_>,
    ) -> zbus::fdo::Result<()> {
        let (service, _) = parse_service(service, hdr, con).await?;
        log::info!("new host: {}", service);

        let added_first = {
            let mut hosts = self.hosts.lock().unwrap();
            if !hosts.insert(service.to_string()) {
                // we're already tracking them
                return Ok(())
            }
            hosts.len() == 1
        };

        if added_first {
            self.is_status_notifier_host_registered_changed(&ctxt).await?;
        }
        Watcher::status_notifier_host_registered(&ctxt).await?;

        self.tasks.spawn({
            let hosts = self.hosts.clone();
            let ctxt = ctxt.to_owned();
            let con = con.to_owned();
            async move {
                wait_for_service_exit(con.clone(), service.as_ref().into()).await.unwrap();
                log::info!("lost host: {}", service);

                let removed_last = {
                    let mut hosts = hosts.lock().unwrap();
                    let did_remove = hosts.remove(service.as_str());
                    did_remove && hosts.is_empty()
                };

                if removed_last {
                    Watcher::is_status_notifier_host_registered_refresh(&ctxt).await.unwrap();
                }
                Watcher::status_notifier_host_unregistered(&ctxt).await.unwrap();
            }
        });

        Ok(())
    }

    /// StatusNotifierHostRegistered signal
    #[dbus_interface(signal)]
    async fn status_notifier_host_registered(ctxt: &zbus::SignalContext<'_>) -> zbus::Result<()>;

    /// StatusNotifierHostUnregistered signal
    #[dbus_interface(signal)]
    async fn status_notifier_host_unregistered(ctxt: &zbus::SignalContext<'_>) -> zbus::Result<()>;

    /// IsStatusNotifierHostRegistered property
    #[dbus_interface(property)]
    async fn is_status_notifier_host_registered(&self) -> bool {
        let hosts = self.hosts.lock().unwrap();
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
            let mut items = self.items.lock().unwrap();
            if !items.insert(item.clone()) {
                // we're already tracking them
                log::info!("new item: {} (duplicate)", item);
                return Ok(())
            }
        }
        log::info!("new item: {}", item);

        self.registered_status_notifier_items_changed(&ctxt).await?;
        Watcher::status_notifier_item_registered(&ctxt, item.as_ref()).await?;

        self.tasks.spawn({
            let items = self.items.clone();
            let ctxt = ctxt.to_owned();
            let con = con.to_owned();
            async move {
                wait_for_service_exit(con.clone(), service.as_ref()).await.unwrap();
                println!("gone item: {}", &item);

                {
                    let mut items = items.lock().unwrap();
                    items.remove(&item);
                }

                Watcher::registered_status_notifier_items_refresh(&ctxt).await.unwrap();
                Watcher::status_notifier_item_unregistered(&ctxt, item.as_ref()).await.unwrap();
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
        let items = self.items.lock().unwrap();
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

    /// Attach the Watcher to a connection.
    pub async fn run_on(self, con: &zbus::Connection) -> zbus::Result<()> {
        if !con.object_server().at(WATCHER_OBJECT_NAME, self).await? {
            return Err(zbus::Error::Failure("Interface already exists at this path".into()))
        }

        // no ReplaceExisting, no AllowReplacement, no DoNotQueue
        con.request_name_with_flags(WATCHER_BUS_NAME, Default::default()).await?;

        Ok(())
    }

    // Based on is_status_notifier_host_registered_invalidate, but without requiring self
    async fn is_status_notifier_host_registered_refresh(ctxt: &zbus::SignalContext<'_>) -> zbus::Result<()> {
        zbus::fdo::Properties::properties_changed(
            ctxt,
            Self::name(),
            &std::collections::HashMap::new(),
            &["IsStatusNotifierHostRegistered"],
        ).await
    }

    // Based on registered_status_notifier_items_invalidate, but without requiring self
    async fn registered_status_notifier_items_refresh(ctxt: &zbus::SignalContext<'_>) -> zbus::Result<()> {
        zbus::fdo::Properties::properties_changed(
            ctxt,
            Self::name(),
            &std::collections::HashMap::new(),
            &["RegisteredStatusNotifierItems"],
        ).await
    }
}

/// Start a StatusNotifierWatcher on this connection.
pub async fn watcher_on(con: &zbus::Connection) -> zbus::Result<()> {
    if !con.object_server().at(WATCHER_OBJECT_NAME, Watcher::new()).await? {
        // There's already something at this object
        // TODO better handling?
        return Err(zbus::Error::Failure(format!("Interface already exists at object {}", WATCHER_OBJECT_NAME)))
    }

    use zbus::fdo::*;
    match con.request_name_with_flags(WATCHER_BUS_NAME, [RequestNameFlags::DoNotQueue].into_iter().collect()).await? {
        RequestNameReply::PrimaryOwner => return Ok(()),
        RequestNameReply::Exists => {},
        RequestNameReply::AlreadyOwner => {}, // TODO should this return
        RequestNameReply::InQueue => unreachable!("request_name_with_flags returned InQueue even though we specified DoNotQueue"),
    }

    // TODO should we queue?

    Ok(())
}
