// NOT GOING TO IMPLEMENT THIS!
// why can't this stupid lib just support this reeeeeeee (https://github.com/GuillaumeGomez/sysinfo/issues/38)
use anyhow::*;
use lazy_static::lazy_static;
use std::{
    fs,
    sync::{Arc, Mutex},
};
use sysinfo::{Disk, DiskExt, RefreshKind, System, SystemExt};

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
        .map(|a: &Disk| {
            std::path::Path::new(a.get_name().clone())
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
        })
        .collect();
    // https://www.kernel.org/doc/Documentation/iostats.txt
    let fields: Vec<&str> = file
        .lines()
        .map(|a| a.trim())
        .filter(|a| disks.contains(&a.split_whitespace().nth(2).unwrap()))
        .collect();
    println!("{:#?}", fields);
    let mut write: usize = 0;
    let mut read: usize = 0;
    for i in fields {
        read = read + i.split_whitespace().nth(5).unwrap().parse::<usize>()?;
        write = write + i.split_whitespace().nth(9).unwrap().parse::<usize>()?;
    }
    Ok((write, read))
}

pub fn get_disk_write() -> Result<f32> {
    let current_values = get_current_values()?;
    println!("current: {:#?}", current_values);
    let tmp = LAST_VALUES.clone();
    println!("last: {:#?}", tmp);
    let mut last_values = tmp.lock().unwrap();
    let usage = current_values.clone().1 as f32 - last_values.1.clone() as f32;
    *last_values = current_values;
    println!("new last: {:#?}", last_values);
    Ok(usage * 4096 as f32)
}
