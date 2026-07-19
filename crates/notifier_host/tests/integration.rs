//! Integration tests against a real DBus session bus.
//!
//! Each test spawns a private `dbus-daemon` so the tests do not depend on the developer's session
//! bus and do not interfere with each other. They are gated behind `#[ignore]` so the default
//! `cargo test` keeps working in environments where `dbus-daemon` is unavailable. Run with:
//!
//! ```bash
//! cargo test -p notifier_host -- --ignored
//! ```

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const WATCHER_BUS: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_OBJECT: &str = "/StatusNotifierWatcher";

// ---------- Temp bus harness ----------

/// A private `dbus-daemon` session bus, killed on drop.
struct TempBus {
    child: Child,
    address: String,
    _config: tempconfig::TempConfig,
}

impl TempBus {
    fn spawn() -> Self {
        let config = tempconfig::TempConfig::write();
        let mut child = Command::new("dbus-daemon")
            .arg("--config-file")
            .arg(config.path())
            .args(["--print-address", "--nofork"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn dbus-daemon; is it installed?");

        let stdout = child.stdout.take().expect("dbus-daemon stdout missing");
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line).expect("failed to read dbus-daemon address");
        let address = line.trim().to_string();
        assert!(!address.is_empty(), "dbus-daemon printed empty address");

        Self { child, address, _config: config }
    }

    async fn connect(&self) -> zbus::Connection {
        zbus::ConnectionBuilder::address(self.address.as_str())
            .expect("invalid bus address")
            .build()
            .await
            .expect("failed to connect to temp bus")
    }
}

impl Drop for TempBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

mod tempconfig {
    use std::io::Write;
    use std::path::{Path, PathBuf};

    pub struct TempConfig {
        path: PathBuf,
    }

    impl TempConfig {
        pub fn write() -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("notifier-host-test-{}-{}.conf", std::process::id(), nanos));
            let contents = r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir=/tmp</listen>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#;
            let mut f = std::fs::File::create(&path).expect("failed to create temp dbus config");
            f.write_all(contents.as_bytes()).expect("failed to write temp dbus config");

            Self { path }
        }

        pub fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempConfig {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

// ---------- Stub watcher interface ----------

struct StubWatcher {
    items: Arc<Mutex<Vec<String>>>,
}

#[zbus::dbus_interface(name = "org.kde.StatusNotifierWatcher")]
impl StubWatcher {
    async fn register_status_notifier_host(&self, _service: &str) -> zbus::fdo::Result<()> {
        Ok(())
    }

    async fn register_status_notifier_item(
        &self,
        service: &str,
        #[zbus(signal_context)] ctxt: zbus::SignalContext<'_>,
    ) -> zbus::fdo::Result<()> {
        self.items.lock().unwrap().push(service.to_string());
        StubWatcher::status_notifier_item_registered(&ctxt, service).await?;

        Ok(())
    }

    #[dbus_interface(signal)]
    async fn status_notifier_item_registered(ctxt: &zbus::SignalContext<'_>, service: &str) -> zbus::Result<()>;

    #[dbus_interface(signal)]
    async fn status_notifier_item_unregistered(ctxt: &zbus::SignalContext<'_>, service: &str) -> zbus::Result<()>;

    #[dbus_interface(property)]
    async fn registered_status_notifier_items(&self) -> Vec<String> {
        self.items.lock().unwrap().clone()
    }

    #[dbus_interface(property)]
    async fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[dbus_interface(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

async fn start_stub_watcher(bus_address: &str, items: Vec<String>) -> zbus::Connection {
    let con = zbus::ConnectionBuilder::address(bus_address)
        .unwrap()
        .build()
        .await
        .expect("stub: failed to connect");
    con.object_server()
        .at(WATCHER_OBJECT, StubWatcher { items: Arc::new(Mutex::new(items)) })
        .await
        .expect("stub: failed to install interface");
    con.request_name_with_flags(
        WATCHER_BUS,
        [zbus::fdo::RequestNameFlags::DoNotQueue].into_iter().collect(),
    )
    .await
    .expect("stub: failed to request name");

    con
}

// ---------- Test host ----------

#[derive(Debug, Clone, PartialEq, Eq)]
enum HostEvent {
    Add(String),
    Remove(String),
    Clear,
}

#[derive(Default)]
struct TestHost {
    events: Arc<Mutex<Vec<HostEvent>>>,
}

impl TestHost {
    fn events_handle(&self) -> Arc<Mutex<Vec<HostEvent>>> {
        self.events.clone()
    }
}

impl notifier_host::Host for TestHost {
    fn add_item(&mut self, id: &str, _item: notifier_host::Item) {
        self.events.lock().unwrap().push(HostEvent::Add(id.to_string()));
    }

    fn remove_item(&mut self, id: &str) {
        self.events.lock().unwrap().push(HostEvent::Remove(id.to_string()));
    }

    fn clear(&mut self) {
        self.events.lock().unwrap().push(HostEvent::Clear);
    }
}

// ---------- Helpers ----------

async fn wait_for<F, Fut, T>(timeout: Duration, mut f: F) -> Option<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if let Some(v) = f().await {
            return Some(v);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    None
}

async fn get_owner(con: &zbus::Connection, name: &str) -> Option<String> {
    let dbus = zbus::fdo::DBusProxy::new(con).await.unwrap();
    match dbus.get_name_owner(zbus::names::BusName::try_from(name).unwrap()).await {
        Ok(o) => Some(o.to_string()),
        Err(zbus::fdo::Error::NameHasNoOwner(_)) => None,
        Err(e) => panic!("unexpected get_name_owner error: {}", e),
    }
}

// ---------- Tests ----------

#[tokio::test]
#[ignore]
async fn watcher_claims_name_when_initial_owner_departs() {
    let bus = TempBus::spawn();

    let stub = start_stub_watcher(&bus.address, vec![]).await;
    let stub_unique = stub.unique_name().unwrap().to_string();

    let eww_con = bus.connect().await;
    notifier_host::Watcher::new().attach_to(&eww_con).await.expect("attach_to failed");

    let observer = bus.connect().await;
    let owner_now = get_owner(&observer, WATCHER_BUS).await;
    assert_eq!(owner_now.as_deref(), Some(stub_unique.as_str()));

    drop(stub);

    let eww_unique = eww_con.unique_name().unwrap().to_string();
    let acquired = wait_for(Duration::from_secs(5), || async {
        let owner = get_owner(&observer, WATCHER_BUS).await;
        (owner.as_deref() == Some(eww_unique.as_str())).then_some(())
    })
    .await;
    assert!(acquired.is_some(), "eww failed to claim {} after stub departed", WATCHER_BUS);
}

#[tokio::test(flavor = "current_thread")]
#[ignore]
async fn run_host_forever_waits_for_initial_owner_then_rebootstraps() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let bus = TempBus::spawn();
            let host_con = bus.connect().await;
            let mut host = TestHost::default();
            let events = host.events_handle();

            let host_task = tokio::task::spawn_local(async move {
                notifier_host::run_host_forever(&mut host, &host_con).await
            });

            // No watcher on the bus: run_host_forever should block, not emit any events.
            tokio::time::sleep(Duration::from_millis(200)).await;
            assert!(events.lock().unwrap().is_empty(), "host should not have received events before any watcher exists");

            // Bring up stub #1. Expect the first Clear (bootstrap).
            let stub1 = start_stub_watcher(&bus.address, vec![]).await;

            let clear_count = |evs: &[HostEvent]| evs.iter().filter(|e| matches!(e, HostEvent::Clear)).count();

            let saw_first_bootstrap = wait_for(Duration::from_secs(5), || async {
                (clear_count(&events.lock().unwrap()) >= 1).then_some(())
            })
            .await;
            assert!(
                saw_first_bootstrap.is_some(),
                "host did not bootstrap after first watcher came up; got {:?}",
                events.lock().unwrap()
            );

            // Swap to stub #2; expect a second Clear from the rebootstrap.
            drop(stub1);
            let _stub2 = start_stub_watcher(&bus.address, vec![]).await;

            let saw_rebootstrap = wait_for(Duration::from_secs(5), || async {
                (clear_count(&events.lock().unwrap()) >= 2).then_some(())
            })
            .await;
            assert!(
                saw_rebootstrap.is_some(),
                "host did not see a second Clear after watcher swap; got {:?}",
                events.lock().unwrap()
            );

            host_task.abort();
        })
        .await;
}

#[tokio::test]
#[ignore]
async fn register_as_host_reuses_single_name_across_calls() {
    let bus = TempBus::spawn();
    let _stub = start_stub_watcher(&bus.address, vec![]).await;
    let con = bus.connect().await;

    // Simulate three open/close cycles on the same (shared) connection.
    let (name1, _) = notifier_host::register_as_host(&con).await.expect("first register");
    let (name2, _) = notifier_host::register_as_host(&con).await.expect("second register");
    let (name3, _) = notifier_host::register_as_host(&con).await.expect("third register");

    // The same well-known name is reused, not incremented (-1, -2, -3).
    assert_eq!(name1, name2);
    assert_eq!(name2, name3);

    // And the bus shows exactly one StatusNotifierHost-* name, not a growing list.
    let dbus = zbus::fdo::DBusProxy::new(&con).await.unwrap();
    let names = dbus.list_names().await.unwrap();
    let host_names: Vec<String> = names
        .iter()
        .map(|n| n.as_str().to_string())
        .filter(|n| n.starts_with("org.freedesktop.StatusNotifierHost-"))
        .collect();

    assert_eq!(host_names.len(), 1, "expected exactly one host name, got {:?}", host_names);
}

#[allow(dead_code)]
const _: () = {
    // Silence unused warnings for HostEvent::Remove if no test uses it.
    let _ = HostEvent::Remove;
};
