use std::{
    collections::HashMap,
    io::BufReader,
    process::{Child, Stdio},
    time::Duration,
};

use crate::{app, config, eww_state, util, value::PrimitiveValue};
use anyhow::*;
use app::EwwCommand;
use glib;
use itertools::Itertools;
use scheduled_executor;
use std::io::BufRead;

/// Handler that manages running and updating [ScriptVar]s
pub struct ScriptVarHandler {
    evt_send: glib::Sender<EwwCommand>,
    pub poll_handles: Vec<scheduled_executor::executor::TaskHandle>,
    pub poll_executor: scheduled_executor::CoreExecutor,
    pub tail_handler_thread: Option<stoppable_thread::StoppableHandle<()>>,
}

impl ScriptVarHandler {
    pub fn new(evt_send: glib::Sender<EwwCommand>) -> Result<Self> {
        log::info!("initializing handler for poll script vars");
        Ok(ScriptVarHandler {
            evt_send,
            poll_handles: Vec::new(),
            poll_executor: scheduled_executor::CoreExecutor::new()?,
            tail_handler_thread: None,
        })
    }

    /// stop all running handlers
    pub fn stop(&mut self) {
        self.poll_handles.iter().for_each(|handle| handle.stop());
        self.poll_handles.clear();
        self.tail_handler_thread.take().map(|handle| handle.stop());
    }

    /// initialize this handler, cleaning up any previously ran executors and
    /// threads.
    pub fn initialize_clean(&mut self, script_vars: Vec<config::ScriptVar>) -> Result<()> {
        self.stop();

        let mut poll_script_vars = Vec::new();
        let mut tail_script_vars = Vec::new();
        for var in script_vars {
            match var {
                config::ScriptVar::Poll(x) => poll_script_vars.push(x),
                config::ScriptVar::Tail(x) => tail_script_vars.push(x),
            }
        }
        self.setup_poll_tasks(&poll_script_vars)?;
        self.setup_tail_tasks(&tail_script_vars)?;
        Ok(())
    }

    /// initialize the poll handler thread.
    fn setup_poll_tasks(&mut self, poll_script_vars: &[config::PollScriptVar]) -> Result<()> {
        log::info!("initializing handler for poll script vars");
        self.poll_handles.iter().for_each(|handle| handle.stop());
        self.poll_handles.clear();

        let evt_send = self.evt_send.clone();
        self.poll_handles = poll_script_vars
            .iter()
            .map(|var| {
                self.poll_executor.schedule_fixed_interval(
                    Duration::from_secs(0),
                    var.interval,
                    glib::clone!(@strong var, @strong evt_send => move |_| {
                        let result: Result<_> = try {
                            let output = eww_state::run_command(&var.command)?;
                            evt_send.send(app::EwwCommand::UpdateVar(var.name.clone(), output))?;
                        };
                        util::print_result_err("while running script-var command", &result);
                    }),
                )
            })
            .collect_vec();
        Ok(())
    }

    /// initialize the tail_var handler thread
    pub fn setup_tail_tasks(&mut self, tail_script_vars: &[config::TailScriptVar]) -> Result<()> {
        log::info!("initializing handler for tail script vars");
        let mut sources = popol::Sources::with_capacity(tail_script_vars.len());

        let mut command_children = Vec::new();
        let mut command_out_handles = HashMap::new();

        for var in tail_script_vars {
            if let Some(mut child) = try_run_command(&var.command) {
                command_out_handles.insert(var.name.clone(), BufReader::new(child.stdout.take().unwrap()));
                command_children.push(child);
            }
        }

        for (var_name, handle) in command_out_handles.iter() {
            sources.register(var_name.clone(), handle.get_ref(), popol::interest::READ);
        }

        let mut events = popol::Events::with_capacity(tail_script_vars.len());
        let evt_send = self.evt_send.clone();
        // TODO this is rather ugly
        let thread_handle = stoppable_thread::spawn(move |stopped| {
            while !stopped.get() {
                let result: Result<_> = try {
                    sources.wait(&mut events)?;
                    for (var_name, event) in events.iter() {
                        if event.readable {
                            let handle = command_out_handles
                                .get_mut(var_name)
                                .with_context(|| format!("No command output handle found for variable '{}'", var_name))?;
                            let mut buffer = String::new();
                            handle.read_line(&mut buffer)?;
                            evt_send.send(EwwCommand::UpdateVar(
                                var_name.clone(),
                                PrimitiveValue::from_string(buffer.trim_matches('\n').to_owned()),
                            ))?;
                        } else if event.hangup {
                            command_out_handles.remove(var_name);
                        }
                    }
                };
                util::print_result_err("in script-var tail handler thread", &result);
            }

            // stop child processes after exit
            for mut child in command_children {
                let _ = child.kill();
            }
        });
        self.tail_handler_thread = Some(thread_handle);
        Ok(())
    }
}

impl Drop for ScriptVarHandler {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Run a command in sh, returning its stdout-handle wrapped in a
/// [`BufReader`]. If running the command fails, will print a warning
/// and return `None`.
fn try_run_command(command: &str) -> Option<Child> {
    let result = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .stdin(Stdio::null())
        .spawn();

    match result {
        Ok(handle) => Some(handle),
        Err(err) => {
            eprintln!("WARN: Error running command from script-variable: {:?}", err);
            None
        }
    }
}
