use lazy_static::lazy_static;
#[cfg(target_os = "macos")]
use regex::Regex;
use std::sync::{Arc, Mutex};
use sysinfo::{Component, ComponentExt, DiskExt, NetworkExt, NetworksExt, ProcessorExt, RefreshKind, System, SystemExt};
lazy_static! {
    static ref SYSTEM: Arc<Mutex<System>> = Arc::new(Mutex::new(System::new()));
    static ref SYSTEM_NET_UP: Arc<Mutex<System>> = Arc::new(Mutex::new(System::new_with_specifics(
        RefreshKind::new().with_networks_list()
    )));
}
use anyhow::*;

pub fn disk() -> Result<f32> {
    let mut c = SYSTEM.lock().unwrap();
    c.refresh_disks_list();

    let root = c
        .get_disks()
        .iter()
        .find(|&x| x.get_mount_point() == std::path::Path::new("/"))
        .ok_or_else(|| anyhow!("Couldn't find a drive mounted at /"))?;
    Ok((root.get_total_space() as f32 - root.get_available_space() as f32) / 1_000_000_000f32)
}

pub fn ram() -> f32 {
    let mut c = SYSTEM.lock().unwrap();
    c.refresh_memory();
    (c.get_used_memory() as f32 + c.get_used_swap() as f32) / 1_000_000f32
}

pub fn cores() -> f32 {
    let mut c = SYSTEM.lock().unwrap();
    c.refresh_components();
    let comp = c.get_components().iter().filter(|&x| x.get_label().starts_with("Core "));
    comp.clone().map(|x| x.get_temperature()).sum::<f32>() / comp.collect::<Vec<&Component>>().len() as f32
}

pub fn get_avg_cpu_usage() -> f32 {
    let mut c = SYSTEM.lock().unwrap();
    c.refresh_cpu();
    c.get_processors().iter().map(|a| a.get_cpu_usage()).sum::<f32>() / c.get_processors().len() as f32
}

#[cfg(target_os = "macos")]
pub fn get_battery_capacity() -> Result<u8> {
    let capacity = String::from_utf8(
        std::process::Command::new("pmset")
            .args(&["-g", "batt"])
            .output()
            .context("\nError while getting the battery value on macos, with `pmset`: ")?
            .stdout,
    )?;

    // Sample output of that command:
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
    Ok(std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity")
        .context("Couldn't get battery info from /sys/class/power_supply/BAT0/capacity")?
        .trim()
        .parse()
        .context("Couldn't parse the number in /sys/class/power_supply/BAT0/capacity")?)
}

#[cfg(not(target_os = "macos"))]
#[cfg(not(target_os = "linux"))]
pub fn get_battery_capacity() -> Result<u8> {
    anyhow!("eww doesn't support your OS for getting the battery capacity")
}

// different pub mods, because if they were reading from the same data, they'd both refresh at the same time,
// and a system to give the data you want, without refreshing constantly would be way out of the scope of this
pub fn get_down() -> f32 {
    let mut c = SYSTEM.lock().unwrap();
    let conn_interfaces = get_interfaces().unwrap_or_else(|_| vec!["docker".to_string(), "lo".to_string()]);
    let interfaces: u64 = c
        .get_networks()
        .iter()
        .filter(|a| conn_interfaces.contains(a.0))
        .map(|a| a.1.get_received())
        .sum();
    println!("{:#?}", interfaces);
    c.refresh_networks_list();
    interfaces as f32 / 1000000 as f32
}

pub fn get_up() -> f32 {
    let mut c = SYSTEM_NET_UP.lock().unwrap();
    let conn_interfaces = get_interfaces().unwrap_or_else(|_| vec!["docker0".to_string(), "lo".to_string()]);
    println!("{:#?}", conn_interfaces);
    let interfaces: u64 = c
        .get_networks()
        .iter()
        .filter(|a| conn_interfaces.contains(a.0))
        .map(|a| a.1.get_transmitted())
        .sum();
    c.refresh_networks_list();
    interfaces as f32 / 1000000 as f32
}

// function to get interfaces, that are connected
#[cfg(target_os = "linux")]
fn get_interfaces() -> Result<Vec<String>> {
    let fs = std::fs::read_to_string("/proc/self/net/route")
        .context("Couldn't open file `/proc/self/net/route`. Super old linux kernel? Disabled procfs?")?;
    let mut interfaces: Vec<String> = Vec::new();
    for i in fs.lines().skip(1) {
        interfaces.push(
            i.split_whitespace()
                .nth(0)
                .ok_or_else(|| anyhow!("Couldn't parse the content of /proc/self/net/route"))?
                .to_string(),
        );
    }

    Ok(interfaces)
}

#[cfg(not(target_os = "linux"))]
fn get_interfaces() -> Result<Vec<String>> {
    anyhow!("eww doesn't support getting the interfaces on your OS")
}
