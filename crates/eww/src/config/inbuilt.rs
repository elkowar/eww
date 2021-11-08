use std::{collections::HashMap, time::Duration};

use simplexpr::{dynval::DynVal, SimplExpr};
use yuck::config::script_var_definition::{PollScriptVar, ScriptVarDefinition, VarSource};

use crate::config::system_stats::*;
use eww_shared_util::VarName;

macro_rules! builtin_vars {
    ($interval:expr, $($name:literal => $fun:expr),*$(,)?) => {{
        maplit::hashmap! {
            $(
            VarName::from($name) => ScriptVarDefinition::Poll(PollScriptVar {
                name: VarName::from($name),
                run_while_expr: SimplExpr::Literal(DynVal::from(true)),
                run_while_var_refs: Vec::new(),
                command: VarSource::Function($fun),
                initial_value: None,
                interval: $interval,
                name_span: eww_shared_util::span::Span::DUMMY,
            })
            ),*
        }
    }}}

pub fn get_inbuilt_vars() -> HashMap<VarName, ScriptVarDefinition> {
    builtin_vars! {Duration::new(2, 0),
        // @desc EWW_TEMPS - Heat of the components in Celcius
        // @prop { <name>: temperature }
        "EWW_TEMPS" => || Ok(DynVal::from(get_temperatures())),

        // @desc EWW_RAM - Information on ram and swap usage in kB.
        // @prop { total_mem, free_mem, total_swap, free_swap, available_mem, used_mem, used_mem_perc }
        "EWW_RAM" => || Ok(DynVal::from(get_ram())),

        // @desc EWW_DISK - Information on on all mounted partitions (Might report inaccurately on some filesystems, like btrfs)\nExample: `{EWW_DISK["/"]}`
        // @prop { <mount_point>: { name, total, free, used, used_perc } }
        "EWW_DISK" => || Ok(DynVal::from(get_disks())),

        // @desc EWW_BATTERY - Battery capacity in procent of the main battery
        // @prop { <name>: { capacity, status } }
        "EWW_BATTERY" => || Ok(DynVal::from(
            match get_battery_capacity() {
                Err(e) => {
                    log::error!("Couldn't get the battery capacity: {:?}", e);
                    "Error: Check `eww log` for more details".to_string()
                }
                Ok(o) => o,
            }
        )),

        // @desc EWW_CPU - Information on the CPU cores: frequency and usage (No MacOS support)
        // @prop { cores: [{ core, freq, usage }], avg }
        "EWW_CPU" => || Ok(DynVal::from(get_cpus())),

        // @desc EWW_NET - Bytes up/down on all interfaces
        // @prop { <name>: { up, down } }
        "EWW_NET" => || Ok(DynVal::from(net())),
    }
}
