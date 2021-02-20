use lazy_static::lazy_static;
use std::sync::{Arc, Mutex};
use sysinfo::{RefreshKind, System, SystemExt};

lazy_static! {
    static ref SYSTEM: Arc<Mutex<System>> = Arc::new(Mutex::new(System::new_with_specifics(RefreshKind::new().with_memory())));
}

pub fn ram() -> f32 {
    let sys = SYSTEM.clone();
    let mut c = sys.lock().unwrap();
    c.refresh_memory();
    (c.get_used_memory() as f32 + c.get_used_swap() as f32) / 1000000 as f32
}
