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
    }
}
