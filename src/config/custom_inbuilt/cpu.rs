use lazy_static::lazy_static;
use std::sync::{Arc, Mutex};
use sysinfo::{ProcessorExt, RefreshKind, System, SystemExt};
lazy_static! {
    static ref SYSTEM: Arc<Mutex<System>> = Arc::new(Mutex::new(System::new_with_specifics(RefreshKind::new().with_cpu())));
}
pub fn get_avg_cpu_usage() -> f32 {
    let sys = SYSTEM.clone();
    let mut c = sys.lock().unwrap();
    c.refresh_cpu();
    c.get_processors().iter().map(|a| a.get_cpu_usage()).sum::<f32>() / c.get_processors().len() as f32
}
