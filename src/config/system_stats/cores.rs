use lazy_static::lazy_static;
use std::sync::{Arc, Mutex};
use sysinfo::{Component, ComponentExt, RefreshKind, System, SystemExt};

lazy_static! {
    static ref SYSTEM: Arc<Mutex<System>> =
        Arc::new(Mutex::new(System::new_with_specifics(RefreshKind::new().with_components())));
}

pub fn cores() -> f32 {
    let sys = SYSTEM.clone();
    let mut c = sys.lock().unwrap();
    c.refresh_components();
    let comp = c.get_components().iter().filter(|&x| x.get_label().starts_with("Core "));
    comp.clone().map(|x| x.get_temperature()).sum::<f32>() / comp.collect::<Vec<&Component>>().len() as f32
}
