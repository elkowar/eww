use itertools::Itertools;
use lazy_static::lazy_static;
use std::sync::{Mutex};
use crate::util::IterAverage;
use sysinfo::{ComponentExt, DiskExt, NetworkExt, NetworksExt, ProcessorExt, RefreshKind, System, SystemExt};
use anyhow::*;

lazy_static! {
    static ref SYSTEM: Mutex<System> = Mutex::new(System::new());
}

pub fn disk() -> Result<f32> {
    let mut c = SYSTEM.lock().unwrap();
    c.refresh_disks_list();

    let root = c
        .get_disks()
        .iter()
        .find(|&x| x.get_mount_point() == std::path::Path::new("/"))
        .context("Couldn't find a drive mounted at /")?;
    Ok((root.get_total_space() as f32 - root.get_available_space() as f32) / 1_000_000_000f32)
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
    let mut components = String::from("{");
    for c in c.get_components() {
        components.push_str(&format!("\"{}\":\"{}\",", c.get_label().to_uppercase().replace(" ", "_"), c.get_temperature()));
    }
    components.pop();
    components.push('}');
    components
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
    let mut interfaces = String::from("{ \"NET_DOWN\": {");
    interfaces.push_str(&c
        .get_networks()
        .iter()
        .map(|a| format!("\"{}\":\"{}\"", a.0, a.1.get_received()))
        .join(","));

    interfaces.push_str("}, \"NET_UP\": {");
    interfaces.push_str(&c
        .get_networks()
        .iter()
        .map(|a| format!("\"{}\":\"{}\"", a.0, a.1.get_transmitted()))
        .join(","));
    interfaces.push_str("}}");
    dbg!(&interfaces);
    c.refresh_networks_list();
    interfaces
}
