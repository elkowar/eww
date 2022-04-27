use std::process::Command;

use eww_shared_util::VarName;
use simplexpr::dynval::DynVal;
use yuck::config::attr_value::ExecutableAction;

use crate::state::scope_graph::{ScopeGraphEvent, ScopeIndex};

pub mod build_widget;
pub mod circular_progressbar;
pub mod def_widget_macro;
pub mod graph;
pub mod transform;
pub mod widget_definitions;

#[macro_export]
macro_rules! action_args {
    ($($key:literal => $value:expr),* $(,)?) => {
        serde_json::json!({
            $($key.to_string(): $value),*
        })
    }
}

/// Run an action
pub(self) fn run_action(
    sender: tokio::sync::mpsc::UnboundedSender<ScopeGraphEvent>,
    scope: ScopeIndex,
    timeout: std::time::Duration,
    action: &ExecutableAction,
    args: &serde_json::Value,
) {
    let result: anyhow::Result<()> = try {
        let event_arg = maplit::hashmap! { VarName("event".to_string()) => DynVal::try_from(args)? };
        match action {
            ExecutableAction::Update(varname, value) => {
                let value = value.eval(&event_arg)?;
                sender.send(ScopeGraphEvent::UpdateValue(scope, varname.clone(), value.clone()))?;
            }
            ExecutableAction::Shell(command) => {
                let command = command.eval(&event_arg)?;
                run_command(timeout, command.to_string());
            }
            ExecutableAction::Noop => {}
        }
    };
    if let Err(e) = result {
        log::error!("{}", e);
    }
}

/// Run a command with a given timeout
fn run_command(timeout: std::time::Duration, cmd: String) {
    use wait_timeout::ChildExt;
    std::thread::spawn(move || {
        log::debug!("Running command from widget: {}", cmd);
        let child = Command::new("/bin/sh").arg("-c").arg(&cmd).spawn();
        match child {
            Ok(mut child) => match child.wait_timeout(timeout) {
                // child timed out
                Ok(None) => {
                    log::warn!(": command {} timed out", &cmd);
                    let _ = child.kill();
                    let _ = child.wait();
                }
                Err(err) => log::error!("Failed to execute command {}: {}", cmd, err),
                Ok(Some(_)) => {}
            },
            Err(err) => log::error!("Failed to launch child process: {}", err),
        }
    });
}
