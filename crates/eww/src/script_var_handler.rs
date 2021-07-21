use std::collections::HashMap;

use crate::{
    app, config,
    value::{PrimVal, VarName},
};
use anyhow::*;
use app::DaemonCommand;

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc::UnboundedSender,
};
use tokio_util::sync::CancellationToken;

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
                    tail_handler: TailVarHandler::new(evt_send.clone())?,
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
    pub fn add(&self, script_var: config::ScriptVar) {
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
    AddVar(config::ScriptVar),
    Stop(VarName),
    StopAll,
}

/// Handler that manages running and updating [ScriptVar]s
struct ScriptVarHandler {
    tail_handler: TailVarHandler,
    poll_handler: PollVarHandler,
}

impl ScriptVarHandler {
    async fn add(&mut self, script_var: config::ScriptVar) {
        match script_var {
            config::ScriptVar::Poll(var) => self.poll_handler.start(var).await,
            config::ScriptVar::Tail(var) => self.tail_handler.start(var).await,
        };
    }

    /// Stop the handler that is responsible for a given variable.
    fn stop_for_variable(&mut self, name: &VarName) -> Result<()> {
        log::debug!("Stopping script var process for variable {}", name);
        self.tail_handler.stop_for_variable(name);
        self.poll_handler.stop_for_variable(name);
        Ok(())
    }

    /// stop all running scripts and schedules
    fn stop_all(&mut self) {
        log::debug!("Stopping script-var-handlers");
        self.tail_handler.stop_all();
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

    async fn start(&mut self, var: config::PollScriptVar) {
        log::debug!("starting poll var {}", &var.name);
        let cancellation_token = CancellationToken::new();
        self.poll_handles.insert(var.name.clone(), cancellation_token.clone());
        let evt_send = self.evt_send.clone();
        tokio::spawn(async move {
            let result: Result<_> = try {
                evt_send.send(app::DaemonCommand::UpdateVars(vec![(var.name.clone(), var.run_once()?)]))?;
            };
            crate::print_result_err!("while running script-var command", &result);

            crate::loop_select_exiting! {
                _ = cancellation_token.cancelled() => break,
                _ = tokio::time::sleep(var.interval) => {
                    let result: Result<_> = try {
                        evt_send.send(app::DaemonCommand::UpdateVars(vec![(var.name.clone(), var.run_once()?)]))?;
                    };
                    crate::print_result_err!("while running script-var command", &result);
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

impl Drop for PollVarHandler {
    fn drop(&mut self) {
        self.stop_all();
    }
}

struct TailVarHandler {
    evt_send: UnboundedSender<DaemonCommand>,
    tail_process_handles: HashMap<VarName, CancellationToken>,
}

impl TailVarHandler {
    fn new(evt_send: UnboundedSender<DaemonCommand>) -> Result<Self> {
        let handler = TailVarHandler { evt_send, tail_process_handles: HashMap::new() };
        Ok(handler)
    }

    async fn start(&mut self, var: config::TailScriptVar) {
        log::debug!("starting poll var {}", &var.name);
        let cancellation_token = CancellationToken::new();
        self.tail_process_handles.insert(var.name.clone(), cancellation_token.clone());

        let evt_send = self.evt_send.clone();
        tokio::spawn(async move {
            crate::try_logging_errors!(format!("Executing tail var command {}", &var.command) =>  {
                let mut handle = tokio::process::Command::new("sh")
                    .args(&["-c", &var.command])
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::inherit())
                    .stdin(std::process::Stdio::null())
                    .spawn()?;
                let mut stdout_lines = BufReader::new(handle.stdout.take().unwrap()).lines();
                crate::loop_select_exiting! {
                    _ = handle.wait() => break,
                    _ = cancellation_token.cancelled() => break,
                    Ok(Some(line)) = stdout_lines.next_line() => {
                        let new_value = PrimVal::from_string(line.to_owned());
                        evt_send.send(DaemonCommand::UpdateVars(vec![(var.name.to_owned(), new_value)]))?;
                    }
                    else => break,
                }
                let _ = handle.kill().await;
            });
        });
    }

    fn stop_for_variable(&mut self, name: &VarName) {
        if let Some(token) = self.tail_process_handles.remove(name) {
            log::debug!("stopped tail var {}", name);
            token.cancel();
        }
    }

    fn stop_all(&mut self) {
        self.tail_process_handles.drain().for_each(|(_, token)| token.cancel());
    }
}

impl Drop for TailVarHandler {
    fn drop(&mut self) {
        self.stop_all();
    }
}
