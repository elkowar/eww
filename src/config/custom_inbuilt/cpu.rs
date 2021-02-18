use anyhow::*;
use std::fs;
pub fn get_cpu_usage() -> Result<Vec<usize>> {
    let file = fs::read_to_string("/proc/stat")
        .context("Couldn't open /proc/stat for reading (Super super old linux kernel? Macos?)")?;
    let info = file.lines().find(|x| x.starts_with("cpu ")).unwrap();
    let values = info.split(" ").skip(2).map(|x| x.parse().unwrap_or_default()).collect();
    Ok(values)
}

pub fn calculate_cpu_usage(data: Vec<usize>, prev_data: Vec<usize>) -> f32 {
    // based on this: https://stackoverflow.com/questions/23367857/accurate-calculation-of-cpu-usage-given-in-percentage-in-linux
    let idle = data[3] + data[4];
    let prev_idle = prev_data[3] + prev_data[4];
    // guest is already calculated in
    let not_idle = data[0] + data[1] + data[2] + data[5] + data[6];
    let prev_not_idle = prev_data[0] + prev_data[1] + prev_data[2] + prev_data[5] + prev_data[6];

    let total = idle + not_idle;
    let prev_total = prev_idle + prev_not_idle;

    let totald = total - prev_total;
    let idled = idle - prev_idle;
    let res = (totald as f32 - idled as f32) / totald as f32;
    res
}
