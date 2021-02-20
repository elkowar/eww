use lazy_static::lazy_static;
use std::sync::{Arc, Mutex};
use sysinfo::{NetworkExt, NetworksExt, RefreshKind, System, SystemExt};

lazy_static! {
    static ref SYSTEM: Arc<Mutex<System>> = Arc::new(Mutex::new(System::new_with_specifics(
        RefreshKind::new().with_networks_list()
    )));
}

pub fn get_up() -> f32 {
    let sys = SYSTEM.clone();
    let mut c = sys.lock().unwrap();
    c.refresh_networks();
    let interfaces: u64 = c
        .get_networks()
        .iter()
        .filter(|a| !a.0.starts_with("docker") || !a.0.starts_with("lo"))
        .map(|a| a.1.get_transmitted())
        .sum();
    interfaces as f32 / 1000000 as f32
}

pub fn get_down() -> f32 {
    let sys = SYSTEM.clone();
    let mut c = sys.lock().unwrap();
    c.refresh_networks();
    let interfaces: u64 = c
        .get_networks()
        .iter()
        .filter(|a| !a.0.starts_with("docker") || !a.0.starts_with("lo"))
        .map(|a| a.1.get_received())
        .sum();
    interfaces as f32 / 1000000 as f32
}
