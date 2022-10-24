use std::collections::HashMap;

use crate::{
    app,
    config::{create_script_var_failed_warn, script_var},
};
use anyhow::{anyhow, Result};
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
    let thread_handle = std::thread::Builder::new()
        .name("outer-script-var-handler".to_string())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_name("script-var-handler")
                .build()
                .expect("Failed to initialize tokio runtime for script var handlers");
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
                                handler.stop_for_variable(&name).await?;
                            }
                            ScriptVarHandlerMsg::StopAll => {
                                handler.stop_all().await;
                                break;
                            }
                        },
                        else => break,
                    };
                };
            })
        })
        .expect("Failed to start script-var-handler thread");
    ScriptVarHandlerHandle { msg_send, thread_handle }
}

/// Handle to the script-var handling system.
pub struct ScriptVarHandlerHandle {
    msg_send: UnboundedSender<ScriptVarHandlerMsg>,
    thread_handle: std::thread::JoinHandle<()>,
}

impl ScriptVarHandlerHandle {
    /// Add a new script-var that should be executed.
    /// This is idempodent, meaning that running a definition that already has a script_var attached which is running
    /// won't do anything.
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

    pub fn join_thread(self) {
        let _ = self.thread_handle.join();
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
    async fn stop_for_variable(&mut self, name: &VarName) -> Result<()> {
        log::debug!("Stopping script var process for variable {}", name);
        self.listen_handler.stop_for_variable(name).await;
        self.poll_handler.stop_for_variable(name);
        Ok(())
    }

    /// stop all running scripts and schedules
    async fn stop_all(&mut self) {
        log::debug!("Stopping script-var-handlers");
        self.listen_handler.stop_all().await;
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
    listen_process_handles: HashMap<VarName, cancellation::AwaitableCancelationSender>,
}

impl ListenVarHandler {
    fn new(evt_send: UnboundedSender<DaemonCommand>) -> Result<Self> {
        let handler = ListenVarHandler { evt_send, listen_process_handles: HashMap::new() };
        Ok(handler)
    }

    /// Start a listen-var. Starting a variable that is already running will not do anything.
    async fn start(&mut self, var: ListenScriptVar) {
        log::debug!("starting listen-var {}", &var.name);

        // Make sure the same listenvar is never started twice,
        // as that would cause eww to not clean up the older listenvar on window close.
        if self.listen_process_handles.contains_key(&var.name) {
            return;
        }

        let (cancel_send, mut cancel_recv) = cancellation::create();
        self.listen_process_handles.insert(var.name.clone(), cancel_send);

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
                let mut completion_notify = None;
                crate::loop_select_exiting! {
                    _ = handle.wait() => break,
                    notify = cancel_recv.wait_for_cancel() => {
                        completion_notify = notify;
                        break;
                    }
                    Ok(Some(line)) = stdout_lines.next_line() => {
                        let new_value = DynVal::from_string(line.to_owned());
                        evt_send.send(DaemonCommand::UpdateVars(vec![(var.name.to_owned(), new_value)]))?;
                    }
                    Ok(Some(line)) = stderr_lines.next_line() => {
                        log::warn!("stderr of `{}`: {}", var.name, line);
                    }
                    else => break,
                };
                terminate_handle(handle).await;

                if let Some(completion_notify) = completion_notify {
                    completion_notify.completed().await;
                }
            });
        });
    }

    async fn stop_for_variable(&mut self, name: &VarName) {
        if let Some(token) = self.listen_process_handles.remove(name) {
            log::debug!("stopped listen-var {}", name);
            token.cancel().await;
        }
    }

    async fn stop_all(&mut self) {
        for (_, token) in self.listen_process_handles.drain() {
            token.cancel().await;
        }
    }
}

impl Drop for ListenVarHandler {
    fn drop(&mut self) {
        if !self.listen_process_handles.is_empty() {
            std::thread::scope(|s| {
                s.spawn(|| {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .thread_name("listen-var-drop-stop-all")
                        .build()
                        .expect("Failed to initialize tokio runtime for script var handlers");
                    rt.block_on(async {
                        self.stop_all().await;
                    });
                });
            })
        }
    }
}

async fn terminate_handle(mut child: tokio::process::Child) {
    if let Some(id) = child.id() {
        log::debug!("Killing process with id {}", id);
        let _ = signal::killpg(Pid::from_raw(id as i32), signal::SIGTERM);
        tokio::select! {
            _ = child.wait() => { },
            _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
                let _ = child.kill().await;
            }
        };
    } else {
        let _ = child.kill().await;
    }
}

// Especially for listenvars, we want to make sure that the scripts are actually
// cancelled before we kill the tokio task that they run in.
// for that, we need to wait for the completion of the cancel itself
/// Provides a CancellationToken-like object that allows to wait for completion of the cancellation.
mod cancellation {
    pub(super) struct CancelCompletionNotifier(tokio::sync::mpsc::Sender<()>);
    impl CancelCompletionNotifier {
        pub async fn completed(self) {
            crate::print_result_err!("Sending cancellation completion", self.0.send(()).await);
        }
    }

    pub(super) struct AwaitableCancelationReceiver(tokio::sync::mpsc::Receiver<CancelCompletionNotifier>);

    impl AwaitableCancelationReceiver {
        pub(super) async fn wait_for_cancel(&mut self) -> Option<CancelCompletionNotifier> {
            self.0.recv().await
        }
    }

    #[derive(Clone)]
    pub(super) struct AwaitableCancelationSender(tokio::sync::mpsc::Sender<CancelCompletionNotifier>);
    impl AwaitableCancelationSender {
        pub(super) async fn cancel(&self) {
            let (send, mut recv) = tokio::sync::mpsc::channel(1);
            if self.0.send(CancelCompletionNotifier(send)).await.is_ok() {
                let _ = recv.recv().await;
            }
        }
    }

    pub(super) fn create() -> (AwaitableCancelationSender, AwaitableCancelationReceiver) {
        let (send, recv) = tokio::sync::mpsc::channel(1);
        (AwaitableCancelationSender(send), AwaitableCancelationReceiver(recv))
    }
}
