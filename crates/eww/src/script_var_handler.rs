use std::collections::HashMap;

use crate::{
    app,
    config::{create_script_var_failed_warn, script_var},
};
use anyhow::*;
use app::DaemonCommand;

use eww_shared_util::VarName;
use nix::{
    sys::signal,
    unistd::{setpgid, Pid},
};
use simplexpr::dynval::DynVal;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc::UnboundedSender,
};
use tokio_util::sync::CancellationToken;
use yuck::config::script_var_definition::{ListenScriptVar, PollScriptVar, ScriptVarDefinition, VarSource};

/// Initialize the script var handler, and return a handle to that handler, which can be used to control
/// the script var execution.
pub fn init(evt_send: UnboundedSender<DaemonCommand>) -> ScriptVarHandlerHandle {
    let (msg_send, mut msg_recv) = tokio::sync::mpsc::unbounded_channel();
    let handle = ScriptVarHandlerHandle { msg_send };
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to initialize tokio runtime for script var handlers");
        rt.block_on(async {
            let _: Result<_> = try {
                let mut handler = ScriptVarHandler {
                    listen_handler: ListenVarHandler::new(evt_send.clone())?,
                    poll_handler: PollVarHandler::new(evt_send)?,
                };
                crate::loop_select_exiting! {
                    Some(msg) = msg_recv.recv() => match msg {
                        ScriptVarHandlerMsg::AddVar(var) => {
                            handler.add(var).await;
                        }
                        ScriptVarHandlerMsg::Stop(name) => {
                            handler.stop_for_variable(&name)?;
                        }
                        ScriptVarHandlerMsg::StopAll => {
                            handler.stop_all();
                        }
                    },
                    else => break,
                };
            };
        })
    });
    handle
}

/// Handle to the script-var handling system.
pub struct ScriptVarHandlerHandle {
    msg_send: UnboundedSender<ScriptVarHandlerMsg>,
}

impl ScriptVarHandlerHandle {
    /// Add a new script-var that should be executed.
    pub fn add(&self, script_var: ScriptVarDefinition) {
        crate::print_result_err!(
            "while forwarding instruction to script-var handler",
            self.msg_send.send(ScriptVarHandlerMsg::AddVar(script_var))
        );
    }

    /// Stop the execution of a specific script-var.
    pub fn stop_for_variable(&self, name: VarName) {
        crate::print_result_err!(
            "while forwarding instruction to script-var handler",
            self.msg_send.send(ScriptVarHandlerMsg::Stop(name)),
        );
    }

    /// Stop the execution of all script-vars.
    pub fn stop_all(&self) {
        crate::print_result_err!(
            "while forwarding instruction to script-var handler",
            self.msg_send.send(ScriptVarHandlerMsg::StopAll)
        );
    }
}

/// Message enum used by the ScriptVarHandlerHandle to communicate to the ScriptVarHandler
#[derive(Debug, Eq, PartialEq)]
enum ScriptVarHandlerMsg {
    AddVar(ScriptVarDefinition),
    Stop(VarName),
    StopAll,
}

/// Handler that manages running and updating [ScriptVarDefinition]s
struct ScriptVarHandler {
    listen_handler: ListenVarHandler,
    poll_handler: PollVarHandler,
}

impl ScriptVarHandler {
    async fn add(&mut self, script_var: ScriptVarDefinition) {
        match script_var {
            ScriptVarDefinition::Poll(var) => self.poll_handler.start(var).await,
            ScriptVarDefinition::Listen(var) => self.listen_handler.start(var).await,
        };
    }

    /// Stop the handler that is responsible for a given variable.
    fn stop_for_variable(&mut self, name: &VarName) -> Result<()> {
        log::debug!("Stopping script var process for variable {}", name);
        self.listen_handler.stop_for_variable(name);
        self.poll_handler.stop_for_variable(name);
        Ok(())
    }

    /// stop all running scripts and schedules
    fn stop_all(&mut self) {
        log::debug!("Stopping script-var-handlers");
        self.listen_handler.stop_all();
        self.poll_handler.stop_all();
    }
}

struct PollVarHandler {
    evt_send: UnboundedSender<DaemonCommand>,
    poll_handles: HashMap<VarName, CancellationToken>,
}

impl PollVarHandler {
    fn new(evt_send: UnboundedSender<DaemonCommand>) -> Result<Self> {
        let handler = PollVarHandler { evt_send, poll_handles: HashMap::new() };
        Ok(handler)
    }

    async fn start(&mut self, var: PollScriptVar) {
        if self.poll_handles.contains_key(&var.name) {
            return;
        }

        log::debug!("starting poll var {}", &var.name);
        let cancellation_token = CancellationToken::new();
        self.poll_handles.insert(var.name.clone(), cancellation_token.clone());
        let evt_send = self.evt_send.clone();
        tokio::spawn(async move {
            let result: Result<_> = try {
                evt_send.send(app::DaemonCommand::UpdateVars(vec![(var.name.clone(), run_poll_once(&var)?)]))?;
            };
            if let Err(err) = result {
                crate::error_handling_ctx::print_error(err);
            }

            crate::loop_select_exiting! {
                _ = cancellation_token.cancelled() => break,
                _ = tokio::time::sleep(var.interval) => {
                    let result: Result<_> = try {
                        evt_send.send(app::DaemonCommand::UpdateVars(vec![(var.name.clone(), run_poll_once(&var)?)]))?;
                    };

                    if let Err(err) = result {
                        crate::error_handling_ctx::print_error(err);
                    }
                }
            }
        });
    }

    fn stop_for_variable(&mut self, name: &VarName) {
        if let Some(token) = self.poll_handles.remove(name) {
            log::debug!("stopped poll var {}", name);
            token.cancel()
        }
    }

    fn stop_all(&mut self) {
        self.poll_handles.drain().for_each(|(_, token)| token.cancel());
    }
}

fn run_poll_once(var: &PollScriptVar) -> Result<DynVal> {
    match &var.command {
        VarSource::Shell(span, command) => {
            script_var::run_command(command).map_err(|e| anyhow!(create_script_var_failed_warn(*span, &var.name, &e.to_string())))
        }
        VarSource::Function(x) => x().map_err(|e| anyhow!(e)),
    }
}

impl Drop for PollVarHandler {
    fn drop(&mut self) {
        self.stop_all();
    }
}

struct ListenVarHandler {
    evt_send: UnboundedSender<DaemonCommand>,
    listen_process_handles: HashMap<VarName, CancellationToken>,
}

impl ListenVarHandler {
    fn new(evt_send: UnboundedSender<DaemonCommand>) -> Result<Self> {
        let handler = ListenVarHandler { evt_send, listen_process_handles: HashMap::new() };
        Ok(handler)
    }

    async fn start(&mut self, var: ListenScriptVar) {
        log::debug!("starting listen-var {}", &var.name);
        let cancellation_token = CancellationToken::new();
        self.listen_process_handles.insert(var.name.clone(), cancellation_token.clone());

        let evt_send = self.evt_send.clone();
        tokio::spawn(async move {
            crate::try_logging_errors!(format!("Executing listen var-command {}", &var.command) =>  {
                let mut handle = unsafe {
                    tokio::process::Command::new("sh")
                    .args(&["-c", &var.command])
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .stdin(std::process::Stdio::null())
                    .pre_exec(|| {
                        let _ = setpgid(Pid::from_raw(0), Pid::from_raw(0));
                        Ok(())
                    }).spawn()?
                };
                let mut stdout_lines = BufReader::new(handle.stdout.take().unwrap()).lines();
                let mut stderr_lines = BufReader::new(handle.stderr.take().unwrap()).lines();
                crate::loop_select_exiting! {
                    _ = handle.wait() => break,
                    _ = cancellation_token.cancelled() => break,
                    Ok(Some(line)) = stdout_lines.next_line() => {
                        let new_value = DynVal::from_string(line.to_owned());
                        evt_send.send(DaemonCommand::UpdateVars(vec![(var.name.to_owned(), new_value)]))?;
                    }
                    Ok(Some(line)) = stderr_lines.next_line() => {
                        log::warn!("stderr of `{}`: {}", var.name, line);
                    }
                    else => break,
                }
                terminate_handle(handle).await;
            });
        });
    }

    fn stop_for_variable(&mut self, name: &VarName) {
        if let Some(token) = self.listen_process_handles.remove(name) {
            log::debug!("stopped listen-var {}", name);
            token.cancel();
        }
    }

    fn stop_all(&mut self) {
        self.listen_process_handles.drain().for_each(|(_, token)| token.cancel());
    }
}

impl Drop for ListenVarHandler {
    fn drop(&mut self) {
        self.stop_all();
    }
}

async fn terminate_handle(mut child: tokio::process::Child) {
    if let Some(id) = child.id() {
        let _ = signal::killpg(Pid::from_raw(id as i32), signal::SIGTERM);
        tokio::select! {
            _ = child.wait() => {},
            _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
                let _ = child.kill().await;
            }
        };
    } else {
        let _ = child.kill().await;
    }
}
