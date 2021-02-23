use crate::{
    config::{
        system_stats::{battery, cores, cpu, disk, network_down, network_up, ram},
        PollScriptVar, ScriptVar,
        VarSource::Function,
    },
    value::{PrimitiveValue, VarName},
};
use anyhow::*;
use std::{collections::HashMap, time::Duration};

pub fn get_inbuilt_vars() -> HashMap<VarName, ScriptVar> {
    let interval = Duration::new(2, 0);

    maplit::hashmap! {
        // @desc EWW_RAM - The current RAM + Swap usage
        VarName::from("EWW_RAM") => ScriptVar::Poll(PollScriptVar {
        name: VarName::from("EWW_RAM"),
        command: Function(|| -> Result<PrimitiveValue, Error> {
            Ok(PrimitiveValue::from(format!("{:.2}", ram::ram())))
        }),
        interval,
    }),
        // @desc EWW_CORES - The average core heat in Celcius
        VarName::from("EWW_CORES") => ScriptVar::Poll(PollScriptVar {
        name: VarName::from("EWW_CORES"),
        command: Function(|| -> Result<PrimitiveValue, Error> {
            Ok(PrimitiveValue::from(format!("{:.1}", cores::cores())))
        }),
        interval,
    }),
        // @desc EWW_DISK - Used space on / in GB (Might report inaccurately on some filesystems, like btrfs)
        VarName::from("EWW_DISK") => ScriptVar::Poll(PollScriptVar {
        name: VarName::from("EWW_DISK"),
        command: Function(|| -> Result<PrimitiveValue, Error> {
            Ok(PrimitiveValue::from(format!("{:.1}",
                match disk::disk() {
                    Err(e) => {log::error!("Couldn't get disk usage on `/`: {:?}", e); f32::NAN}
                    Ok(o) => o
                }
            )))
        }),
        interval,
    }),
        // @desc EWW_BATTERY - Battery capacity in procent of the main battery
        VarName::from("EWW_BATTERY") => ScriptVar::Poll(PollScriptVar {
        name: VarName::from("EWW_BATTERY"),
        command: Function(|| -> Result<PrimitiveValue, Error> {
            Ok(PrimitiveValue::from(
                match battery::get_battery_capacity() {
                    Err(e) => {log::error!("Couldn't get the battery capacity: {:?}", e); f32::NAN }
                    Ok(o) => o as f32,
                }
            ))
        }),
        interval,
    }),
        // @desc EWW_CPU - Average CPU usage (all cores) in the last two seconds (No MacOS support)
        VarName::from("EWW_CPU") => ScriptVar::Poll(PollScriptVar {
        name: VarName::from("EWW_CPU"),
        command: Function(|| -> Result<PrimitiveValue, Error> {
            Ok(PrimitiveValue::from(format!("{:.2}", cpu::get_avg_cpu_usage())))
        }),
        interval,
    }),
        // @desc EWW_NET_UP - Megabyte uploaded on interface since last update (excluding the docker and local one)
        VarName::from("EWW_NET_UP") => ScriptVar::Poll(PollScriptVar {
        name: VarName::from("EWW_NET_UP"),
        command: Function(|| -> Result<PrimitiveValue, Error> {
            Ok(PrimitiveValue::from(format!("{:.2}", network_up::get_up())))
        }),
        interval,
    }),
        // @desc EWW_NET_DOWN - Megabyte downloaded on all interfaces since last update (excluding the docker and local one)
        VarName::from("EWW_NET_DOWN") => ScriptVar::Poll(PollScriptVar {
        name: VarName::from("EWW_NET_DOWN"),
        command: Function(|| -> Result<PrimitiveValue, Error> {
            Ok(PrimitiveValue::from(format!("{:.2}", network_down::get_down())))
        }),
        interval,
    }),
    }
}
