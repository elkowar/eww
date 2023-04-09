use zbus::dbus_interface;
use zbus::Interface;
use zbus::export::ordered_stream::OrderedStreamExt;

pub const WATCHER_BUS_NAME: &'static str = "org.kde.StatusNotifierWatcher";
pub const WATCHER_OBJECT_NAME: &'static str = "/StatusNotifierWatcher";

fn parse_service(service: &str) -> (Option<zbus::names::BusName<'_>>, &str) {
    if service.starts_with("/") {
        // they sent us just the object path :(
        (None, service)
    } else {
        // should be a bus name
        (service.try_into().ok(), "/StatusNotifierItem")
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
        let (service, _) = parse_service(service);
        let service = if let Some(x) = service {
            x.to_owned()
        } else if let Some(sender) = hdr.sender()? {
            sender.to_owned().into()
        } else {
            log::warn!("register_status_notifier_host: unknown sender");
            return Err(zbus::fdo::Error::InvalidArgs("Unknown bus address".into()));
        };
        log::info!("new host: {}", service);

        {
            let mut hosts = self.hosts.lock().unwrap();
            if !hosts.insert(service.to_string()) {
                // we're already tracking them
                return Ok(())
            }
        }

        self.is_status_notifier_host_registered_changed(&ctxt).await?;
        Watcher::status_notifier_host_registered(&ctxt).await?;

        self.tasks.spawn({
            let hosts = self.hosts.clone();
            let ctxt = ctxt.to_owned();
            let con = con.to_owned();
            async move {
                wait_for_service_exit(con.clone(), service.as_ref()).await.unwrap();
                log::info!("lost host: {}", service);

                {
                    let mut hosts = hosts.lock().unwrap();
                    hosts.remove(service.as_str());
                }

                Watcher::is_status_notifier_host_registered_refresh(&ctxt).await.unwrap();
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
        let (service, objpath) = parse_service(service);
        let service: zbus::names::UniqueName<'_> = if let Some(x) = service {
            let dbus = zbus::fdo::DBusProxy::new(&con).await?;
            dbus.get_name_owner(x).await?.into_inner()
        } else if let Some(sender) = hdr.sender()? {
            sender.to_owned()
        } else {
            log::warn!("register_status_notifier_item: unknown sender");
            return Err(zbus::fdo::Error::InvalidArgs("Unknown bus address".into()));
        };
        let service = zbus::names::BusName::Unique(service);

        let item = format!("{}{}", service, objpath);
        log::info!("new item: {}", item);

        {
            let mut items = self.items.lock().unwrap();
            if !items.insert(item.clone()) {
                // we're already tracking them
                return Ok(())
            }
        }

        self.registered_status_notifier_items_changed(&ctxt).await?;
        Watcher::status_notifier_item_registered(&ctxt, service.as_ref()).await?;

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
                Watcher::status_notifier_item_unregistered(&ctxt, service.as_ref()).await.unwrap();
            }
        });

        Ok(())
    }

    /// StatusNotifierItemRegistered signal
    #[dbus_interface(signal)]
    async fn status_notifier_item_registered(ctxt: &zbus::SignalContext<'_>, service: zbus::names::BusName<'_>) -> zbus::Result<()>;

    /// StatusNotifierItemUnregistered signal
    #[dbus_interface(signal)]
    async fn status_notifier_item_unregistered(ctxt: &zbus::SignalContext<'_>, service: zbus::names::BusName<'_>) -> zbus::Result<()>;

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
        RequestNameReply::InQueue => panic!("request_name_with_flags returned InQueue even though we specified DoNotQueue"),
    }

    // TODO should we queue?

    Ok(())
}
