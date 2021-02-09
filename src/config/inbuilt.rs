// DON'T REMOVE THIS!
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
            let r = RefreshKind::new();
            let r = r.with_memory();
            let c: System = System::new_with_specifics(r);
            Ok(PrimitiveValue::from(format!(
                "{:.2}",
                ((c.get_used_memory() as f32 + c.get_used_swap() as f32) / 1000000 as f32)
            )))
        }),
        interval,
    }),

    }
}
