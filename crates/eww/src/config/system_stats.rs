use crate::util::IterAverage;
use anyhow::{Context, Result};
use itertools::Itertools;
use once_cell::sync::Lazy;
use std::{fs::read_to_string, sync::Mutex};
use sysinfo::{ComponentExt, CpuExt, DiskExt, NetworkExt, NetworksExt, System, SystemExt};
use local_ip_address::list_afinet_netifas;


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
    let json = serde_json::json!({
        "cores": cpus.iter()
            .map(|a| {
                serde_json::json!({
                    "core": a.name(),
                    "freq": a.frequency(),
                    "usage": a.cpu_usage() as i64
                })
            }).collect::<Vec<_>>(),
        "avg": cpus.iter().map(|a| a.cpu_usage()).avg()
    });
    serde_json::to_string(&json).unwrap()
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
    use std::{collections::HashMap, sync::atomic::AtomicBool};

    #[derive(serde::Serialize)]
    struct BatteryData {
        capacity: i64,
        status: String,
    }

    #[derive(serde::Serialize)]
    struct Data {
        #[serde(flatten)]
        batteries: HashMap<String, BatteryData>,
        total_avg: f64,
    }

    let mut current = 0_f64;
    let mut total = 0_f64;
    let mut batteries = HashMap::new();
    let power_supply_dir = std::path::Path::new("/sys/class/power_supply");
    let power_supply_entries = power_supply_dir.read_dir().context("Couldn't read /sys/class/power_supply directory")?;
    for entry in power_supply_entries {
        let entry = entry?.path();
        if !entry.is_dir() {
            continue;
        }
        if let (Ok(capacity), Ok(status)) = (read_to_string(entry.join("capacity")), read_to_string(entry.join("status"))) {
            batteries.insert(
                entry.file_name().context("Couldn't get filename")?.to_string_lossy().to_string(),
                BatteryData {
                    status: status.trim_end_matches('\n').to_string(),
                    capacity: capacity.trim_end_matches('\n').parse::<f64>()?.round() as i64,
                },
            );
            if let (Ok(charge_full), Ok(charge_now), Ok(voltage_now)) = (
                read_to_string(entry.join("charge_full")),
                read_to_string(entry.join("charge_now")),
                read_to_string(entry.join("voltage_now")),
            ) {
                // (uAh / 1000000) * U = p and that / one million so that we have microwatt
                current += ((charge_now.trim_end_matches('\n').parse::<f64>()? / 1000000_f64)
                    * voltage_now.trim_end_matches('\n').parse::<f64>()?)
                    / 1000000_f64;
                total += ((charge_full.trim_end_matches('\n').parse::<f64>()? / 1000000_f64)
                    * voltage_now.trim_end_matches('\n').parse::<f64>()?)
                    / 1000000_f64;
            } else if let (Ok(energy_full), Ok(energy_now)) =
                (read_to_string(entry.join("energy_full")), read_to_string(entry.join("energy_now")))
            {
                current += energy_now.trim_end_matches('\n').parse::<f64>()?;
                total += energy_full.trim_end_matches('\n').parse::<f64>()?;
            } else {
                static WARNED: AtomicBool = AtomicBool::new(false);
                if !WARNED.load(std::sync::atomic::Ordering::Relaxed) {
                    WARNED.store(true, std::sync::atomic::Ordering::Relaxed);
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

    Ok(serde_json::to_string(&(Data { batteries, total_avg: (current / total) * 100_f64 })).unwrap())
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

pub fn get_time() -> String {
    chrono::offset::Utc::now().timestamp().to_string()
}

pub fn get_ipv4() -> String {
    let ifas = list_afinet_netifas().unwrap();
    let joined = ifas
    .iter()
    .filter(|ipv| ipv.1.is_ipv4() && ipv.0 != "lo")
    .map(|ip| format!("{}", ip.1))
    .collect::<Vec<_>>()
    .join(", ");
    joined
}

pub fn get_ipv6() -> String {
    let ifas = list_afinet_netifas().unwrap();
    let joined = ifas
    .iter()
    .filter(|ipv| ipv.1.is_ipv6() && ipv.0 != "lo")
    .map(|ip| format!("{}", ip.1))
    .collect::<Vec<_>>()
    .join(", ");
    joined
}