use std::process::Command;

use anyhow::*;
use eww_shared_util::{Span, VarName};
use simplexpr::dynval::DynVal;
use yuck::{
    config::script_var_definition::{ScriptVarDefinition, VarSource},
    gen_diagnostic,
};

use crate::error::DiagError;

pub fn create_script_var_failed_error(span: Span, var_name: &VarName) -> DiagError {
    DiagError::new(gen_diagnostic! {
        msg = format!("Failed to compute value for `{}`", var_name),
        label = span => "Defined here",
    })
}

pub fn initial_value(var: &ScriptVarDefinition) -> Result<DynVal> {
    match var {
        ScriptVarDefinition::Poll(x) => match &x.command {
            VarSource::Function(f) => {
                f().map_err(|err| anyhow!(err)).with_context(|| format!("Failed to compute initial value for {}", &var.name()))
            }
            VarSource::Shell(span, f) => run_command(f).map_err(|_| anyhow!(create_script_var_failed_error(*span, var.name()))),
        },
        ScriptVarDefinition::Listen(_) => Ok(DynVal::from_string(String::new())),
    }
}

/// Run a command and get the output
pub fn run_command(cmd: &str) -> Result<DynVal> {
    log::debug!("Running command: {}", cmd);
    let command = Command::new("/bin/sh").arg("-c").arg(cmd).output()?;
    if !command.status.success() {
        bail!("Execution of `{}` failed", cmd);
    }
    let output = String::from_utf8(command.stdout)?;
    let output = output.trim_matches('\n');
    Ok(DynVal::from(output))
}
