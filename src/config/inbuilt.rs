// Network and CPU load have to be done with the *async magic*
use crate::{
    config::{PollScriptVar, ScriptVar},
    value::{PrimitiveValue, VarName},
};
use anyhow::*;
use std::{collections::HashMap, time::Duration};
use sysinfo::*;

pub fn get_inbuilt_vars() -> HashMap<VarName, ScriptVar> {
    let interval = Duration::new(2, 0);

    maplit::hashmap! {
         // @desc EWW_RAM_USAGE - The current RAM + Swap usage
         VarName::from("EWW_RAM_USAGE") => ScriptVar::Poll(PollScriptVar {
         name: VarName::from("EWW_RAM_USAGE"),
         command: crate::config::VarSource::Function(|| -> Result<PrimitiveValue, Error> {
             let r = RefreshKind::new().with_memory();
             let c: System = System::new_with_specifics(r);
             Ok(PrimitiveValue::from(format!(
                 // converts it to GB and only displays two fraction digits
                 "{:.2}",
                 ((c.get_used_memory() as f32 + c.get_used_swap() as f32) / 1000000 as f32)
             )))
         }),
         interval,
     }),
         // @desc EWW_CORES - The average core heat in Celcius
         VarName::from("EWW_CORES") => ScriptVar::Poll(PollScriptVar {
         name: VarName::from("EWW_CORES"),
         command: crate::config::VarSource::Function(|| -> Result<PrimitiveValue, Error> {
             let r = RefreshKind::new().with_components_list();
             let c = System::new_with_specifics(r);
             let c = c.get_components();
             let cores = c.iter().filter(|&x| x.get_label().starts_with("Core "));
             Ok(PrimitiveValue::from(format!("{:.1}", cores.clone().map(|x| x.get_temperature()).sum::<f32>() / cores.collect::<Vec<&Component>>().len() as f32)))
         }),
         interval,
     }),
        // @desc EWW_DISK - Used space on / in GB (Might report inaccurately on some filesystems, like btrfs)
        VarName::from("EWW_DISK") => ScriptVar::Poll(PollScriptVar {
        name: VarName::from("EWW_DISK"),
        command: crate::config::VarSource::Function(|| -> Result<PrimitiveValue, Error> {
             let r = RefreshKind::new().with_disks_list();
             let c = System::new_with_specifics(r);
             let c = c.get_disks();
             let root = c.iter().find(|&x| x.get_mount_point() == std::path::Path::new("/")).unwrap(); // unwrap because i'm pretty positive there's always a /
             Ok(PrimitiveValue::from(format!("{:.1}", (root.get_total_space() as f32 - root.get_available_space() as f32) / 1000000000 as f32)))
         }),
         interval,
     }),
        // @desc EWW_BATTERY - Battery capacity in procent of the main battery
        VarName::from("EWW_BATTERY") => ScriptVar::Poll(PollScriptVar {
        name: VarName::from("EWW_BATTERY"),
        command: crate::config::VarSource::Function(|| -> Result<PrimitiveValue, Error> {
            Ok(PrimitiveValue::from(
            // this is f32 so that we can use NaN, if there's an error
           match crate::config::custom_inbuilt::battery::get_battery_capacity() {
                Err(e) => {log::error!("Couldn't get the battery capacity: {:?}", e); f32::NAN }
                Ok(o) => o as f32,
            }
            ))
         }),
         interval,
     }),
        // @desc EWW_CPU - Average CPU usage (all cores) in the last two seconds
        VarName::from("EWW_CPU") => ScriptVar::Poll(PollScriptVar {
        name: VarName::from("EWW_CPU"),
        command: crate::config::VarSource::Function(|| -> Result<PrimitiveValue, Error> {
            Ok(PrimitiveValue::from(format!("{:.2}",
           match crate::config::custom_inbuilt::cpu::get_cpu_usage() {
                Err(e) => {log::error!("Couldn't get the cpu usage: {:?}", e); f32::NAN }
                Ok(o) => {
                    crate::config::custom_inbuilt::cpu::calculate_cpu_usage(o, vec![1 , 1, 1, 1, 1, 1 ,1 ,1 ,1 ,1 ,1 ])
                },
            }
            )))
         }),
         interval,
     }),

    }
}
