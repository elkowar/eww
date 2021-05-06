use crate::util::IterAverage;
use anyhow::*;
use itertools::Itertools;
use lazy_static::lazy_static;
use std::sync::Mutex;
use sysinfo::{ComponentExt, DiskExt, NetworkExt, NetworksExt, ProcessorExt, System, SystemExt};

lazy_static! {
    static ref SYSTEM: Mutex<System> = Mutex::new(System::new());
}

pub fn disk() -> String {
    let mut c = SYSTEM.lock().unwrap();
    c.refresh_disks_list();

    format!(
        "{{ {} }}",
        c.get_disks().iter().map(|c| format!(
            r#""{}": {{"name": {:?}, "total": {}, "free": {}}}"#,
            c.get_mount_point().display(),
            c.get_name(),
            c.get_total_space(),
            c.get_available_space(),
        )).join(",")
    )
}

pub fn ram() -> f32 {
    let mut c = SYSTEM.lock().unwrap();
    c.refresh_memory();
    (c.get_used_memory() as f32 + c.get_used_swap() as f32) / 1_000_000f32
}

pub fn cores() -> String {
    let mut c = SYSTEM.lock().unwrap();
    c.refresh_components_list();
    c.refresh_components();
    format!(
        "{{ {} }}",
        c.get_components()
            .iter()
            .map(|c| format!(r#""{}": {}"#, c.get_label().to_uppercase().replace(" ", "_"), c.get_temperature()))
            .join(",")
    )
}

pub fn get_avg_cpu_usage() -> f32 {
    let mut c = SYSTEM.lock().unwrap();
    c.refresh_cpu();
    c.get_processors().iter().map(|a| a.get_cpu_usage()).avg()
}

#[cfg(target_os = "macos")]
pub fn get_battery_capacity() -> Result<u8> {
    use regex::Regex;
    let capacity = String::from_utf8(
        std::process::Command::new("pmset")
            .args(&["-g", "batt"])
            .output()
            .context("\nError while getting the battery value on macos, with `pmset`: ")?
            .stdout,
    )?;

    // Example output of that command:
    // Now drawing from 'Battery Power'
    //-InternalBattery-0 (id=11403363)	100%; discharging; (no estimate) present: true
    let regex = Regex::new(r"[0-9]*%")?;
    let mut number = regex.captures(&capacity).unwrap().get(0).unwrap().as_str().to_string();
    // Removes the % at the end
    number.pop();
    Ok(number.parse().context("Couldn't make a number from the parsed text")?)
}

#[cfg(target_os = "linux")]
pub fn get_battery_capacity() -> Result<u8> {
    std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity")
        .context("Couldn't get battery info from /sys/class/power_supply/BAT0/capacity")?
        .trim()
        .parse()
        .context("Couldn't parse the number in /sys/class/power_supply/BAT0/capacity")
}

#[cfg(not(target_os = "macos"))]
#[cfg(not(target_os = "linux"))]
pub fn get_battery_capacity() -> Result<u8> {
    anyhow!("eww doesn't support your OS for getting the battery capacity")
}

pub fn net() -> String {
    let mut c = SYSTEM.lock().unwrap();
    let interfaces = format!(
        "{{ {} }}",
        &c.get_networks()
            .iter()
            .map(|a| format!(r#""{}": {{ "NET_UP": {}, "NET_DOWN": {} }}"#, a.0, a.1.get_transmitted(), a.1.get_received()))
            .join(","),
    );
    c.refresh_networks_list();
    interfaces
}
