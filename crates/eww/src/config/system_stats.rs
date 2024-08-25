use crate::util::IterAverage;
use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use std::{fs::read_to_string, sync::Mutex};
use sysinfo::System;

struct RefreshTime(std::time::Instant);
impl RefreshTime {
    pub fn new() -> Self {
        Self(std::time::Instant::now())
    }

    pub fn next_refresh(&mut self) -> std::time::Duration {
        let now = std::time::Instant::now();
        let duration = now.duration_since(self.0);
        self.0 = now;
        duration
    }
}

static SYSTEM: Lazy<Mutex<System>> = Lazy::new(|| Mutex::new(System::new()));
static DISKS: Lazy<Mutex<sysinfo::Disks>> = Lazy::new(|| Mutex::new(sysinfo::Disks::new_with_refreshed_list()));
static COMPONENTS: Lazy<Mutex<sysinfo::Components>> = Lazy::new(|| Mutex::new(sysinfo::Components::new_with_refreshed_list()));
static NETWORKS: Lazy<Mutex<(RefreshTime, sysinfo::Networks)>> =
    Lazy::new(|| Mutex::new((RefreshTime::new(), sysinfo::Networks::new_with_refreshed_list())));

pub fn get_disks() -> String {
    let mut disks = DISKS.lock().unwrap();
    disks.refresh_list();
    disks.refresh();

    disks
        .iter()
        .map(|c| {
            let total_space = c.total_space();
            let available_space = c.available_space();
            let used_space = total_space - available_space;

            (
                c.mount_point().display().to_string(),
                serde_json::json!({
                    "name": c.name(),
                    "total": total_space,
                    "free": available_space,
                    "used": used_space,
                    "used_perc": (used_space as f32 / total_space as f32) * 100f32
                }),
            )
        })
        .collect::<serde_json::Value>()
        .to_string()
}

pub fn get_ram() -> String {
    let mut system = SYSTEM.lock().unwrap();
    system.refresh_memory();

    let total_memory = system.total_memory();
    let available_memory = system.available_memory();
    let used_memory = total_memory as f32 - available_memory as f32;
    serde_json::json!({
        "total_mem": total_memory,
        "free_mem": system.free_memory(),
        "total_swap": system.total_swap(),
        "free_swap": system.free_swap(),
        "available_mem": available_memory,
        "used_mem": used_memory,
        "used_mem_perc": (used_memory / total_memory as f32) * 100f32,
    })
    .to_string()
}

pub fn get_temperatures() -> String {
    let mut components = COMPONENTS.lock().unwrap();
    components.refresh_list();
    components.refresh();
    components
        .iter()
        .map(|c| {
            (
                c.label().to_uppercase().replace(' ', "_"),
                // It is common for temperatures to report a non-numeric value.
                // Tolerate it by serializing it as the string "null"
                c.temperature().to_string().replace("NaN", "\"null\""),
            )
        })
        .collect::<serde_json::Value>()
        .to_string()
}

pub fn get_cpus() -> String {
    let mut system = SYSTEM.lock().unwrap();
    system.refresh_cpu_specifics(sysinfo::CpuRefreshKind::everything());
    let cpus = system.cpus();
    serde_json::json!({
        "cores": cpus.iter()
            .map(|a| {
                serde_json::json!({
                    "core": a.name(),
                    "freq": a.frequency(),
                    "usage": a.cpu_usage() as i64
                })
            }).collect::<Vec<_>>(),
        "avg": cpus.iter().map(|a| a.cpu_usage()).avg()
    })
    .to_string()
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

#[cfg(any(target_os = "netbsd", target_os = "freebsd", target_os = "openbsd"))]
pub fn get_battery_capacity() -> Result<String> {
    let batteries = String::from_utf8(
        // I have only tested `apm` on FreeBSD, but it *should* work on all of the listed targets,
        // based on what I can tell from their online man pages.
        std::process::Command::new("apm")
            .output()
            .context("\nError while getting the battery values on bsd, with `apm`: ")?
            .stdout,
    )?;

    // `apm` output should look something like this:
    // $ apm
    // ...
    // Remaining battery life: 87%
    // Remaining battery time: unknown
    // Number of batteries: 1
    // Battery 0
    //         Battery Status: charging
    //         Remaining battery life: 87%
    //         Remaining battery time: unknown
    // ...
    // last 4 lines are repeated for each battery.
    // see also:
    // https://www.freebsd.org/cgi/man.cgi?query=apm&manpath=FreeBSD+13.1-RELEASE+and+Ports
    // https://man.openbsd.org/amd64/apm.8
    // https://man.netbsd.org/apm.8
    let mut json = String::from('{');
    let re_total = regex!(r"(?m)^Remaining battery life: (\d+)%");
    let re_single = regex!(r"(?sm)^Battery (\d+):.*?Status: (\w+).*?(\d+)%");
    for bat in re_single.captures_iter(&batteries) {
        json.push_str(&format!(
            r#""BAT{}": {{ "status": "{}", "capacity": {} }}, "#,
            bat.get(1).unwrap().as_str(),
            bat.get(2).unwrap().as_str(),
            bat.get(3).unwrap().as_str(),
        ))
    }

    json.push_str(&format!(r#""total_avg": {}}}"#, re_total.captures(&batteries).unwrap().get(1).unwrap().as_str()));
    Ok(json)
}

#[cfg(not(target_os = "macos"))]
#[cfg(not(target_os = "linux"))]
#[cfg(not(target_os = "netbsd"))]
#[cfg(not(target_os = "freebsd"))]
#[cfg(not(target_os = "openbsd"))]
pub fn get_battery_capacity() -> Result<String> {
    Err(anyhow::anyhow!("Eww doesn't support your OS for getting the battery capacity"))
}

pub fn net() -> String {
    let (ref mut last_refresh, ref mut networks) = &mut *NETWORKS.lock().unwrap();

    networks.refresh_list();
    let elapsed = last_refresh.next_refresh();

    networks
        .iter()
        .map(|(name, data)| {
            let transmitted = data.transmitted() as f64 / elapsed.as_secs_f64();
            let received = data.received() as f64 / elapsed.as_secs_f64();
            (name, serde_json::json!({ "NET_UP": transmitted, "NET_DOWN": received }))
        })
        .collect::<serde_json::Value>()
        .to_string()
}

pub fn get_time() -> String {
    chrono::offset::Utc::now().timestamp().to_string()
}
