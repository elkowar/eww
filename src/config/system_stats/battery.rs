use anyhow::*;

#[cfg(target_os = "macos")]
use regex::Regex;

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
    anyhow!("EWW doesn't support your OS for getting the battery capacity")
}
