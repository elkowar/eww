use std::collections::HashMap;

use crate::{
    app, config,
    value::{PrimitiveValue, VarName},
};
use anyhow::*;
use app::EwwCommand;

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc::UnboundedSender,
};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Eq, PartialEq)]
enum ScriptVarHandlerMsg {
    AddVar(config::ScriptVar),
    Stop(VarName),
    StopAll,
}

pub struct ScriptVarHandlerHandle {
    msg_send: UnboundedSender<ScriptVarHandlerMsg>,
}

impl ScriptVarHandlerHandle {
    pub fn add(&self, script_var: config::ScriptVar) {
        self.msg_send.send(ScriptVarHandlerMsg::AddVar(script_var)).unwrap();
    }

    pub fn stop_for_variable(&self, name: VarName) {
        self.msg_send.send(ScriptVarHandlerMsg::Stop(name)).unwrap();
    }

    pub fn stop_all(&self) {
        self.msg_send.send(ScriptVarHandlerMsg::StopAll).unwrap();
    }
}

/// Handler that manages running and updating [ScriptVar]s
struct ScriptVarHandler {
    tail_handler: TailVarHandler,
    poll_handler: PollVarHandler,
}

pub fn init(evt_send: UnboundedSender<EwwCommand>) -> Result<ScriptVarHandlerHandle> {
    let (msg_send, mut msg_recv) = tokio::sync::mpsc::unbounded_channel();
    let handle = ScriptVarHandlerHandle { msg_send };
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let _: Result<_> = try {
                let mut handler = ScriptVarHandler {
                    tail_handler: TailVarHandler::new(evt_send.clone())?,
                    poll_handler: PollVarHandler::new(evt_send)?,
                };
                while let Some(msg) = msg_recv.recv().await {
                    match msg {
                        ScriptVarHandlerMsg::AddVar(var) => {
                            handler.add(var).await;
                        }
                        ScriptVarHandlerMsg::Stop(name) => {
                            handler.stop_for_variable(&name)?;
                        }
                        ScriptVarHandlerMsg::StopAll => {
                            handler.stop_all();
                        }
                    }
                }
            };
        })
    });
    Ok(handle)
}

impl ScriptVarHandler {
    pub async fn add(&mut self, script_var: config::ScriptVar) {
        match script_var {
            config::ScriptVar::Poll(var) => self.poll_handler.start(var).await,
            config::ScriptVar::Tail(var) => self.tail_handler.start(var).await,
        };
    }

    /// Stop the handler that is responsible for a given variable.
    pub fn stop_for_variable(&mut self, name: &VarName) -> Result<()> {
        log::debug!("Stopping script var process for variable {}", name);
        self.tail_handler.stop_for_variable(name)?;
        self.poll_handler.stop_for_variable(name)?;
        Ok(())
    }

    /// stop all running scripts and schedules
    pub fn stop_all(&mut self) {
        log::debug!("Stopping script-var-handlers");
        self.tail_handler.stop_all();
        self.poll_handler.stop_all();
    }
}

impl Drop for ScriptVarHandler {
    fn drop(&mut self) {
        self.stop_all();
    }
}

struct PollVarHandler {
    evt_send: UnboundedSender<EwwCommand>,
    poll_handles: HashMap<VarName, CancellationToken>,
}

impl PollVarHandler {
    fn new(evt_send: UnboundedSender<EwwCommand>) -> Result<Self> {
        let handler = PollVarHandler {
            evt_send,
            poll_handles: HashMap::new(),
        };
        Ok(handler)
    }

    async fn start(&mut self, var: config::PollScriptVar) {
        log::debug!("starting poll var {}", &var.name);
        let cancellation_token = CancellationToken::new();
        self.poll_handles.insert(var.name.clone(), cancellation_token.clone());
        let evt_send = self.evt_send.clone();
        tokio::spawn(async move {
            crate::loop_select! {
                _ = cancellation_token.cancelled() => break,
                _ = tokio::time::sleep(var.interval) => {
                    let result: Result<_> = try {
                        evt_send.send(app::EwwCommand::UpdateVars(vec![(var.name.clone(), var.run_once()?)]))?;
                    };
                    crate::print_result_err!("while running script-var command", &result);
                }
            }
        });
    }

    pub fn stop_for_variable(&mut self, name: &VarName) -> Result<()> {
        if let Some(token) = self.poll_handles.remove(name) {
            log::debug!("stopped poll var {}", name);
            token.cancel()
        }
        Ok(())
    }

    pub fn stop_all(&mut self) {
        self.poll_handles.drain().for_each(|(_, token)| token.cancel());
    }
}

struct TailVarHandler {
    evt_send: UnboundedSender<EwwCommand>,
    tail_process_handles: HashMap<VarName, CancellationToken>,
}

impl TailVarHandler {
    fn new(evt_send: UnboundedSender<EwwCommand>) -> Result<Self> {
        let handler = TailVarHandler {
            evt_send,
            tail_process_handles: HashMap::new(),
        };
        Ok(handler)
    }

    async fn start(&mut self, var: config::TailScriptVar) {
        log::debug!("starting poll var {}", &var.name);
        let cancellation_token = CancellationToken::new();
        self.tail_process_handles.insert(var.name.clone(), cancellation_token.clone());

        let evt_send = self.evt_send.clone();
        tokio::spawn(async move {
            let mut handle = tokio::process::Command::new("sh")
                .args(&["-c", &var.command])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::inherit())
                .stdin(std::process::Stdio::null())
                .spawn()
                .unwrap();
            let mut stdout_lines = BufReader::new(handle.stdout.take().unwrap()).lines();
            crate::loop_select! {
                _ = handle.wait() => break,
                _ = cancellation_token.cancelled() => break,
                line = stdout_lines.next_line() => match line {
                    Ok(Some(line)) => {
                        let new_value = PrimitiveValue::from_string(line.to_owned());
                        evt_send.send(EwwCommand::UpdateVars(vec![(var.name.to_owned(), new_value)])).unwrap();
                    },
                    Ok(None) => break,
                    Err(_e) => break,
                }
            }
        });
    }

    fn stop_for_variable(&mut self, name: &VarName) -> Result<()> {
        if let Some(token) = self.tail_process_handles.remove(name) {
            log::debug!("stopped tail var {}", name);
            token.cancel();
        }
        Ok(())
    }

    fn stop_all(&mut self) {
        self.tail_process_handles.drain().for_each(|(_, token)| token.cancel());
    }
}

pub mod script_var_process {
    use anyhow::*;
    use nix::{
        sys::{signal, wait},
        unistd::Pid,
    };
    use std::{ffi::CString, io::BufReader, sync::Mutex};

    lazy_static::lazy_static! {
        static ref SCRIPT_VAR_CHILDREN: Mutex<Vec<u32>> = Mutex::new(Vec::new());
    }

    fn terminate_pid(pid: u32) -> Result<()> {
        signal::kill(Pid::from_raw(pid as i32), signal::SIGTERM)?;
        wait::waitpid(Pid::from_raw(pid as i32), None)?;
        Ok(())
    }

    /// This function should be called in the signal handler, killing all child processes.
    pub fn on_application_death() {
        SCRIPT_VAR_CHILDREN.lock().unwrap().drain(..).for_each(|pid| {
            let result = terminate_pid(pid);
            crate::print_result_err!("While killing process '{}' during cleanup", &result);
        });
    }

    pub struct ScriptVarProcess {
        pub pid: i32,
        pub stdout_reader: BufReader<filedescriptor::FileDescriptor>,
    }

    impl ScriptVarProcess {
        pub(super) fn run(command: &str) -> Result<Self> {
            use nix::unistd::*;
            use std::os::unix::io::AsRawFd;

            let pipe = filedescriptor::Pipe::new()?;

            match unsafe { fork()? } {
                ForkResult::Parent { child, .. } => {
                    SCRIPT_VAR_CHILDREN.lock().unwrap().push(child.as_raw() as u32);

                    close(pipe.write.as_raw_fd())?;

                    Ok(ScriptVarProcess {
                        stdout_reader: BufReader::new(pipe.read),
                        pid: child.as_raw(),
                    })
                }
                ForkResult::Child => {
                    let _ = setpgid(Pid::from_raw(0), Pid::from_raw(0));
                    match unsafe { fork()? } {
                        ForkResult::Parent { .. } => {
                            simple_signal::set_handler(&[simple_signal::Signal::Int, simple_signal::Signal::Term], |_| {
                                let pgid = getpgid(Some(getpid())).unwrap();
                                let _ = signal::killpg(pgid, nix::sys::signal::SIGKILL);
                                while nix::sys::wait::wait().unwrap().pid().is_some() {}
                            });
                            loop {}
                        }
                        ForkResult::Child => {
                            close(pipe.read.as_raw_fd()).unwrap();
                            dup2(pipe.write.as_raw_fd(), std::io::stdout().as_raw_fd()).unwrap();
                            dup2(pipe.write.as_raw_fd(), std::io::stderr().as_raw_fd()).unwrap();
                            execv(
                                CString::new("/bin/sh").unwrap().as_ref(),
                                &[
                                    CString::new("/bin/sh").unwrap(),
                                    CString::new("-c").unwrap(),
                                    CString::new(command).unwrap(),
                                ],
                            )
                            .unwrap();
                            unreachable!(
                                "Child fork called exec, thus the process was replaced by the command the user provided"
                            );
                        }
                    }
                }
            }
        }

        pub(super) fn kill(&self) -> Result<()> {
            SCRIPT_VAR_CHILDREN.lock().unwrap().retain(|item| *item != self.pid as u32);
            terminate_pid(self.pid as u32).context("Error manually killing tail-var script")
        }
    }

    impl Drop for ScriptVarProcess {
        fn drop(&mut self) {
            let _ = self.kill();
        }
    }
}
