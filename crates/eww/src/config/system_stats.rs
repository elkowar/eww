use crate::util::IterAverage;
use anyhow::*;
use itertools::Itertools;
use lazy_static::lazy_static;
use std::{fs::read_to_string, sync::Mutex};
use sysinfo::{ComponentExt, DiskExt, NetworkExt, NetworksExt, ProcessorExt, System, SystemExt};

lazy_static! {
    static ref SYSTEM: Mutex<System> = Mutex::new(System::new());
}

pub fn disk() -> String {
    let mut c = SYSTEM.lock().unwrap();
    c.refresh_disks_list();

    format!(
        "{{ {} }}",
        c.get_disks()
            .iter()
            .map(|c| format!(
                r#""{}": {{"name": {:?}, "total": {}, "free": {}}}"#,
                c.get_mount_point().display(),
                c.get_name(),
                c.get_total_space(),
                c.get_available_space(),
            ))
            .join(",")
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

pub fn get_avg_cpu_usage() -> String {
    let mut c = SYSTEM.lock().unwrap();
    c.refresh_cpu();
    let processors = c.get_processors();
    format!(
        r#"{{ "cores": [{}], "avg": {} }}"#,
        processors
            .iter()
            .map(|a| format!(r#"{{"core": "{}", "freq": {}, "usage": {}}}"#, a.get_name(), a.get_frequency(), a.get_cpu_usage()))
            .join(","),
        processors.iter().map(|a| a.get_cpu_usage()).avg()
    )
}

#[cfg(target_os = "macos")]
pub fn get_battery_capacity() -> Result<String> {
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
    Ok(format!(
        "{{ \"BAT0\": {{ \"capacity\": \"{}\", \"status\": \"{}\" }}}}",
        number,
        capacity.split(";").collect::<Vec<&str>>()[1]
    ))
}

#[cfg(target_os = "linux")]
pub fn get_battery_capacity() -> Result<String> {
    let mut current = 0_f64;
    let mut total = 0_f64;
    let mut json = String::from('{');
    for i in
        std::path::Path::new("/sys/class/power_supply").read_dir().context("Couldn't read /sys/class/power_supply directory")?
    {
        let i = i?.path();
        if i.is_dir() {
            // some ugly hack because if let Some(a) = a && Some(b) = b doesn't work yet
            if let (Ok(o), Ok(s)) = (read_to_string(i.join("capacity")), read_to_string(i.join("status"))) {
                json.push_str(&format!(
                    r#"{:?}: {{ "status": "{}", "capacity": {} }},"#,
                    i.file_name().context("couldn't convert file name to rust string")?,
                    s.replace("\n", ""),
                    o.replace("\n", "")
                ));
                if let (Ok(t), Ok(c), Ok(v)) = (
                    read_to_string(i.join("charge_full")),
                    read_to_string(i.join("charge_now")),
                    read_to_string(i.join("voltage_now")),
                ) {
                    // (uAh / 1000000) * U = p and that / one million so that we have microwatt
                    current +=
                        ((c.replace("\n", "").parse::<f64>()? / 1000000_f64) * v.replace("\n", "").parse::<f64>()?) / 1000000_f64;
                    total +=
                        ((t.replace("\n", "").parse::<f64>()? / 1000000_f64) * v.replace("\n", "").parse::<f64>()?) / 1000000_f64;
                } else if let (Ok(t), Ok(c)) = (read_to_string(i.join("energy_full")), read_to_string(i.join("energy_now"))) {
                    current += c.replace("\n", "").parse::<f64>()?;
                    total += t.replace("\n", "").parse::<f64>()?;
                } else {
                    log::warn!(
                        "Failed to get/calculate uWh: the total_avg value of the battery magic var will probably be a garbage \
                         value that can not be trusted."
                    );
                }
            }
        }
    }
    json.pop();
    json.push_str(&format!(r#", "total_avg": {:.1}}}"#, (current / total) * 100_f64));

    Ok(json)
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
