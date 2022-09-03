use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use codespan_reporting::diagnostic::Severity;
use eww_shared_util::{Span, VarName};
use simplexpr::dynval::DynVal;
use yuck::{
    config::script_var_definition::{ScriptVarDefinition, VarSource},
    error::DiagError,
    gen_diagnostic,
};

pub fn create_script_var_failed_warn(span: Span, var_name: &VarName, error_output: &str) -> DiagError {
    DiagError(gen_diagnostic! {
        kind = Severity::Warning,
        msg = format!("The script for the `{}`-variable exited unsuccessfully", var_name),
        label = span => "Defined here",
        note = error_output,
    })
}

pub fn initial_value(var: &ScriptVarDefinition) -> Result<DynVal> {
    match var {
        ScriptVarDefinition::Poll(x) => match &x.initial_value {
            Some(value) => Ok(value.clone()),
            None => match &x.command {
                VarSource::Function(f) => f()
                    .map_err(|err| anyhow!(err))
                    .with_context(|| format!("Failed to compute initial value for {}", &var.name())),
                VarSource::Shell(span, command) => {
                    run_command(command).map_err(|e| anyhow!(create_script_var_failed_warn(*span, var.name(), &e.to_string())))
                }
            },
        },

        ScriptVarDefinition::Listen(var) => Ok(var.initial_value.clone()),
    }
}

/// Run a command and get the output
pub fn run_command(cmd: &str) -> Result<DynVal> {
    log::debug!("Running command: {}", cmd);
    let command = Command::new("/bin/sh").arg("-c").arg(cmd).output()?;
    if !command.status.success() {
        bail!("Failed with output:\n{}", String::from_utf8(command.stderr)?);
    }
    let output = String::from_utf8(command.stdout)?;
    let output = output.trim_matches('\n');
    Ok(DynVal::from(output))
}
