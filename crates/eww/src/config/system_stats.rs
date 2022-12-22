use crate::util::IterAverage;
use anyhow::{Context, Result};
use itertools::Itertools;
use once_cell::sync::Lazy;
use std::{fs::read_to_string, sync::Mutex};
use sysinfo::{ComponentExt, CpuExt, DiskExt, NetworkExt, NetworksExt, System, SystemExt};

static SYSTEM: Lazy<Mutex<System>> = Lazy::new(|| Mutex::new(System::new()));

pub fn get_disks() -> String {
    let mut c = SYSTEM.lock().unwrap();
    c.refresh_disks_list();

    format!(
        "{{ {} }}",
        c.disks()
            .iter()
            .map(|c| {
                let total_space = c.total_space();
                let available_space = c.available_space();
                let used_space = total_space - available_space;
                format!(
                    r#""{}": {{"name": {:?}, "total": {}, "free": {}, "used": {}, "used_perc": {}}}"#,
                    c.mount_point().display(),
                    c.name(),
                    total_space,
                    available_space,
                    used_space,
                    (used_space as f32 / total_space as f32) * 100f32,
                )
            })
            .join(",")
    )
}

pub fn get_ram() -> String {
    let mut c = SYSTEM.lock().unwrap();
    c.refresh_memory();

    let total_memory = c.total_memory();
    let available_memory = c.available_memory();
    let used_memory = total_memory as f32 - available_memory as f32;
    format!(
        r#"{{"total_mem": {}, "free_mem": {}, "total_swap": {}, "free_swap": {}, "available_mem": {}, "used_mem": {}, "used_mem_perc": {}}}"#,
        total_memory,
        c.free_memory(),
        c.total_swap(),
        c.free_swap(),
        available_memory,
        used_memory,
        (used_memory / total_memory as f32) * 100f32,
    )
}

pub fn get_temperatures() -> String {
    let mut c = SYSTEM.lock().unwrap();
    c.refresh_components_list();
    c.refresh_components();
    format!(
        "{{ {} }}",
        c.components()
            .iter()
            .map(|c| format!(
                r#""{}": {}"#,
                c.label().to_uppercase().replace(' ', "_"),
                // It is common for temperatures to report a non-numeric value.
                // Tolerate it by serializing it as the string "null"
                c.temperature().to_string().replace("NaN", "\"null\"")
            ))
            .join(",")
    )
}

pub fn get_cpus() -> String {
    let mut c = SYSTEM.lock().unwrap();
    c.refresh_cpu_specifics(sysinfo::CpuRefreshKind::everything());
    let cpus = c.cpus();
    format!(
        r#"{{ "cores": [{}], "avg": {} }}"#,
        cpus.iter()
            .map(|a| format!(r#"{{"core": "{}", "freq": {}, "usage": {:.0}}}"#, a.name(), a.frequency(), a.cpu_usage()))
            .join(","),
        cpus.iter().map(|a| a.cpu_usage()).avg()
    )
}

#[cfg(target_os = "macos")]
pub fn get_battery_capacity() -> Result<String> {
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
    let regex = regex!(r"[0-9]*%");
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
                    s.trim_end_matches(|c| c == '\n'),
                    o.trim_end_matches(|c| c == '\n')
                ));
                if let (Ok(t), Ok(c), Ok(v)) = (
                    read_to_string(i.join("charge_full")),
                    read_to_string(i.join("charge_now")),
                    read_to_string(i.join("voltage_now")),
                ) {
                    // (uAh / 1000000) * U = p and that / one million so that we have microwatt
                    current += ((c.trim_end_matches(|c| c == '\n').parse::<f64>()? / 1000000_f64)
                        * v.trim_end_matches(|c| c == '\n').parse::<f64>()?)
                        / 1000000_f64;
                    total += ((t.trim_end_matches(|c| c == '\n').parse::<f64>()? / 1000000_f64)
                        * v.trim_end_matches(|c| c == '\n').parse::<f64>()?)
                        / 1000000_f64;
                } else if let (Ok(t), Ok(c)) = (read_to_string(i.join("energy_full")), read_to_string(i.join("energy_now"))) {
                    current += c.trim_end_matches(|c| c == '\n').parse::<f64>()?;
                    total += t.trim_end_matches(|c| c == '\n').parse::<f64>()?;
                } else {
                    log::warn!(
                        "Failed to get/calculate uWh: the total_avg value of the battery magic var will probably be a garbage \
                         value that can not be trusted."
                    );
                }
            }
        }
    }
    if total == 0_f64 {
        return Ok(String::from(""));
    }

    json.push_str(&format!(r#" "total_avg": {:.1}}}"#, (current / total) * 100_f64));
    Ok(json)
}

#[cfg(not(target_os = "macos"))]
#[cfg(not(target_os = "linux"))]
pub fn get_battery_capacity() -> Result<String> {
    Err(anyhow::anyhow!("Eww doesn't support your OS for getting the battery capacity"))
}

pub fn net() -> String {
    let mut c = SYSTEM.lock().unwrap();
    c.refresh_networks_list();
    let interfaces = format!(
        "{{ {} }}",
        &c.networks()
            .iter()
            .map(|a| format!(r#""{}": {{ "NET_UP": {}, "NET_DOWN": {} }}"#, a.0, a.1.transmitted(), a.1.received()))
            .join(","),
    );
    interfaces
}
