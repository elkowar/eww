use anyhow::*;

#[cfg(target_os = "macos")]
use regex::Regex;

pub fn get_battery_capacity() -> Result<u8> {
    #[cfg(target_os = "linux")]
    let capacity = linux()?;

    #[cfg(target_os = "macos")]
    let capacity = macos()?;

    #[cfg(not(target_os = "macos"))]
    #[cfg(not(target_os = "linux"))]
    return anyhow!("Not supported OS");

    Ok(capacity)
}

#[cfg(target_os = "macos")]
fn macos() -> Result<u8> {
    let capacity = String::from_utf8(
        std::process::Command::new("pmset")
            .args(&["-g", "batt"])
            .output()
            .context("\nError while getting the battery value on macos, with `pmset`: ")?
            .stdout,
    )?;
    let regex = Regex::new(r"[0-9]*%")?;
    let mut number = regex.captures(&capacity).unwrap().get(0).unwrap().as_str().to_string();
    number.pop();
    Ok(number.parse().context("Couldn't make a number from the parsed text")?)
}

fn linux() -> Result<u8> {
    Ok(std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity")
        .context("Couldn't get battery info from /sys/class/power_supply/BAT0/capacity")?
        .trim()
        .parse()
        .context("Couldn't parse the number in /sys/class/power_supply/BAT0/capacity")?)
}
