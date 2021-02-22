use anyhow::*;
use lazy_static::lazy_static;
use std::sync::{Arc, Mutex};
use sysinfo::{DiskExt, RefreshKind, System, SystemExt};

lazy_static! {
    static ref SYSTEM: Arc<Mutex<System>> =
        Arc::new(Mutex::new(System::new_with_specifics(RefreshKind::new().with_disks_list())));
}

pub fn disk() -> Result<f32> {
    let sys = SYSTEM.clone();
    let mut c = sys.lock().unwrap();
    c.refresh_disks_list();

    let root = c
        .get_disks()
        .iter()
        .find(|&x| x.get_mount_point() == std::path::Path::new("/"))
        .ok_or_else(|| anyhow!("Couldn't find a drive mounted at /"))?;
    Ok((root.get_total_space() as f32 - root.get_available_space() as f32) / 1_000_000_000f32)
}
