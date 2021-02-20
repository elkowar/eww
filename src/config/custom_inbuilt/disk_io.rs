// why can't this stupid lib just support this reeeeeeee (https://github.com/GuillaumeGomez/sysinfo/issues/38)
use anyhow::*;
use lazy_static::lazy_static;
use std::{
    fs,
    sync::{Arc, Mutex},
};
use sysinfo::{DiskExt, RefreshKind, System, SystemExt};

lazy_static! {
    static ref LAST_VALUES: Arc<Mutex<(usize, usize)>> = Arc::new(Mutex::new((0, 0)));
    static ref DISKS: Arc<Mutex<System>> = Arc::new(Mutex::new(System::new_with_specifics(RefreshKind::new().with_disks_list())));
}
fn get_current_values() -> Result<(usize, usize)> {
    let file: String = fs::read_to_string("/proc/diskstats")
        .context("Couldn't open /proc/diskstats for reading (Super super old linux kernel? Macos?)")?;
    let disks = DISKS.clone();
    let mut disks = disks.lock().unwrap();
    disks.refresh_disks_list();
    let disks: Vec<&str> = disks
        .get_disks()
        .iter()
        .map(|a| a.get_mount_point().file_name().unwrap().to_str().unwrap())
        .collect();
    // https://www.kernel.org/doc/Documentation/iostats.txt
    let fields: Vec<&str> = file
        .lines()
        .filter(|a| disks.contains(&a.split_whitespace().nth(2).unwrap()))
        .collect();
    let mut values: Vec<(usize, usize)> = Vec::new();
    for i in fields {
        let read = i.split_whitespace().nth(5).unwrap().parse()?;
        let write = i.split_whitespace().nth(9).unwrap().parse()?;
        values.push((read, write));
    }
    Ok((1, 1))
}

pub fn get_disk_write() -> Result<f32> {
    let current_values = get_current_values()?;
    let tmp = LAST_VALUES.clone();
    let mut last_values = tmp.lock().unwrap();
    let usage = current_values.clone().1 as f32 - last_values.1.clone() as f32;
    *last_values = current_values;
    Ok(usage * 496)
}
