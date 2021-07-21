use std::process::Command;

use anyhow::*;
use simplexpr::dynval::DynVal;
use yuck::config::script_var_definition::{ScriptVarDefinition, VarSource};

pub fn initial_value(var: &ScriptVarDefinition) -> Result<DynVal> {
    match var {
        ScriptVarDefinition::Poll(x) => match &x.command {
            VarSource::Function(f) => {
                f().map_err(|err| anyhow!(err)).with_context(|| format!("Failed to compute initial value for {}", &var.name()))
            }
            VarSource::Shell(f) => run_command(f).with_context(|| format!("Failed to compute initial value for {}", &var.name())),
        },
        ScriptVarDefinition::Tail(_) => Ok(DynVal::from_string(String::new())),
    }
}
/// Run a command and get the output
pub fn run_command(cmd: &str) -> Result<DynVal> {
    log::debug!("Running command: {}", cmd);
    let output = String::from_utf8(Command::new("/bin/sh").arg("-c").arg(cmd).output()?.stdout)?;
    let output = output.trim_matches('\n');
    Ok(DynVal::from(output))
}
