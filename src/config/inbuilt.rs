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
        // @desc EWW_TEMPS - Heat of the components in Celcius\nExample: `{{(CPU_TEMPS.core_1 + CPU_TEMPS.core_2) / 2}}`
        "EWW_TEMPS" => || Ok(PrimitiveValue::from(cores())),

        // @desc EWW_RAM - The current RAM + Swap usage
        "EWW_RAM" => || Ok(PrimitiveValue::from(format!("{:.2}", ram()))),

        // @desc EWW_DISK - Information on on all mounted partitions (Might report inaccurately on some filesystems, like btrfs)\nExample: `{{EWW_DISK["/"]}}`
        "EWW_DISK" => || Ok(PrimitiveValue::from(disk())),

        // @desc EWW_BATTERY - Battery capacity in procent of the main battery
        "EWW_BATTERY" => || Ok(PrimitiveValue::from(
            match get_battery_capacity() {
                Err(e) => {
                    log::error!("Couldn't get the battery capacity: {:?}", e);
                    "Error: Check `eww log` for more details".to_string()
                }
                Ok(o) => o,
            }
        )),

        // @desc EWW_CPU_USAGE - Average CPU usage (all cores) since the last update (No MacOS support)
        "EWW_CPU_USAGE" => || Ok(PrimitiveValue::from(get_avg_cpu_usage())),

        // @desc EWW_NET - Bytes up/down on all interfaces
        "EWW_NET" => || Ok(PrimitiveValue::from(net())),
    }
}
