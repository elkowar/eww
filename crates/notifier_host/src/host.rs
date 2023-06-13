use crate::*;

use zbus::export::ordered_stream::{self, OrderedStreamExt};

pub trait Host {
    fn add_item(&mut self, id: &str, item: Item);
    fn remove_item(&mut self, id: &str);
}

// Attach to dbus and forward events to Host.
//
// This task is blocking and won't return unless an error occurs.
pub async fn serve(host: &mut dyn Host, id: &str) -> Result<()> {
    // From <https://www.freedesktop.org/wiki/Specifications/StatusNotifierItem/StatusNotifierHost/>:
    //
    // Instances of this service are registered on the Dbus session bus, under a name on the
    // form org.freedesktop.StatusNotifierHost-id where id is an unique identifier, that keeps
    // the names unique on the bus, such as the process-id of the application or another type
    // of identifier if more that one StatusNotifierHost is registered by the same process.

    let wellknown_name = format!("org.freedesktop.StatusNotifierHost-{}-{}", std::process::id(), id);
    let con = zbus::ConnectionBuilder::session()?
        .name(wellknown_name.as_str())?
        .build()
        .await?;

    // register ourself to StatusNotifierWatcher
    let snw = dbus::StatusNotifierWatcherProxy::new(&con).await?;
    snw.register_status_notifier_host(&wellknown_name).await?;

    // initial items first
    for svc in snw.registered_status_notifier_items().await? {
        let item = Item::from_address(&con, &svc).await?;
        host.add_item(&svc, item);
    }

    // TODO this is a race condition? we might miss items that appear at this time

    enum ItemEvent {
        NewItem(dbus::StatusNotifierItemRegistered),
        GoneItem(dbus::StatusNotifierItemUnregistered),
    }

    let new_items = snw.receive_status_notifier_item_registered().await?;
    let gone_items = snw.receive_status_notifier_item_unregistered().await?;
    let mut ev_stream = ordered_stream::join(
        OrderedStreamExt::map(new_items, ItemEvent::NewItem),
        OrderedStreamExt::map(gone_items, ItemEvent::GoneItem),
    );
    while let Some(ev) = ev_stream.next().await {
        match ev {
            ItemEvent::NewItem(sig) => {
                let args = sig.args()?;
                let item = Item::from_address(&con, args.service).await?;
                host.add_item(args.service, item);
            },
            ItemEvent::GoneItem(sig) => {
                let args = sig.args()?;
                host.remove_item(args.service);
            },
        }
    }

    Ok(())
}
