use crate::{
    config::{system_stats::*, PollScriptVar, ScriptVar, VarSource},
    value::{PrimVal as PrimitiveValue, VarName},
};
use std::{collections::HashMap, time::Duration};

macro_rules! builtin_vars {
    ($interval:expr, $($name:literal => $fun:expr),*$(,)?) => {{
        maplit::hashmap! {
            $(
            VarName::from($name) => ScriptVar::Poll(PollScriptVar {
                name: VarName::from($name),
                command: VarSource::Function($fun),
                interval: $interval,
            })
            ),*
        }
    }}}

pub fn get_inbuilt_vars() -> HashMap<VarName, ScriptVar> {
    builtin_vars! {Duration::new(2, 0),
        // @desc EWW_HEAT - The average core heat in Celcius. Since it's a JSON value you have to pick the value you want with .core_0, all lowercase and spaces replaced with _. Average core heat can be built with eww expressions: `{{(CPU_TEMPS.core_1 + CPU_TEMPS.core_2) / 2}}`
        "EWW_HEAT" => || Ok(PrimitiveValue::from(cores())),

        // @desc EWW_RAM - The current RAM + Swap usage
        "EWW_RAM" => || Ok(PrimitiveValue::from(format!("{:.2}", ram()))),

        // @desc EWW_DISK - Used space on / in GB (Might report inaccurately on some filesystems, like btrfs)
        "EWW_DISK" => || Ok(PrimitiveValue::from(format!("{:.1}",
            match disk() {
                Err(e) => {
                    log::error!("Couldn't get disk usage on `/`: {:?}", e);
                    f32::NAN
                }
                Ok(o) => o
            }
        ))),

        // @desc EWW_BATTERY - Battery capacity in procent of the main battery
        "EWW_BATTERY" => || Ok(PrimitiveValue::from(
            match get_battery_capacity() {
                Err(e) => {
                    log::error!("Couldn't get the battery capacity: {:?}", e);
                    f32::NAN
                }
                Ok(o) => o as f32,
            }
        )),


        // @desc EWW_CPU - Average CPU usage (all cores) in the last two seconds (No MacOS support)
        "EWW_CPU" => || Ok(PrimitiveValue::from(format!("{:.2}", get_avg_cpu_usage()))),

        // @desc EWW_NET_UP - Megabyte uploaded on all interfaces that have a routing table since last update
        "EWW_NET_UP" => || Ok(PrimitiveValue::from(format!("{:.2}", get_up()))),

        // @desc EWW_NET_DOWN - Megabyte downloaded on all interfaces that have a routing table since last update
        "EWW_NET_DOWN" => || Ok(PrimitiveValue::from(format!("{:.2}", get_down())))
    }
}
