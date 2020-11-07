use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use crate::{
    app, config, util,
    value::{PrimitiveValue, VarName},
};
use anyhow::*;
use app::EwwCommand;
use dashmap::DashMap;
use glib;
use scheduled_executor;
use std::io::BufRead;

use self::script_var_process::ScriptVarProcess;

/// Handler that manages running and updating [ScriptVar]s
pub struct ScriptVarHandler {
    evt_send: glib::Sender<EwwCommand>,
    poll_handles: HashMap<VarName, scheduled_executor::executor::TaskHandle>,
    poll_executor: scheduled_executor::CoreExecutor,
    tail_handler_thread: Option<stoppable_thread::StoppableHandle<()>>,
    tail_process_handles: Arc<DashMap<VarName, script_var_process::ScriptVarProcess>>,
    tail_sources: Arc<RwLock<popol::Sources<VarName>>>,
}

impl ScriptVarHandler {
    pub fn new(evt_send: glib::Sender<EwwCommand>) -> Result<Self> {
        log::info!("initializing handler for poll script vars");
        let mut handler = ScriptVarHandler {
            evt_send,
            poll_handles: HashMap::new(),
            poll_executor: scheduled_executor::CoreExecutor::new()?,
            tail_handler_thread: None,
            tail_process_handles: Arc::new(DashMap::new()),
            tail_sources: Arc::new(RwLock::new(popol::Sources::new())),
        };
        handler.setup_tail_tasks()?;
        Ok(handler)
    }

    pub fn add(&mut self, script_var: config::ScriptVar) {
        match script_var {
            config::ScriptVar::Poll(var) => {
                self.schedule_poll_task(&var);
            }
            config::ScriptVar::Tail(var) => {
                self.start_tail_script(&var);
            }
        };
    }

    /// Stop the handler that is responsible for a given variable.
    pub fn stop_for_variable(&mut self, name: &VarName) -> Result<()> {
        log::debug!("Stopping script var process for variable {}", name);
        if let Some(handle) = self.poll_handles.remove(name) {
            log::debug!("stopped poll var {}", name);
            handle.stop();
        }
        if let Some((_, process)) = self.tail_process_handles.remove(name) {
            log::debug!("stopped tail var {}", name);
            process.kill()?;
        }
        Ok(())
    }

    /// stop all running handlers
    pub fn stop(&mut self) {
        self.poll_handles.drain().for_each(|(_, handle)| handle.stop());
        self.tail_handler_thread.take().map(|handle| handle.stop());
    }

    fn schedule_poll_task(&mut self, var: &config::PollScriptVar) {
        let evt_send = self.evt_send.clone();
        let handle = self.poll_executor.schedule_fixed_interval(
            Duration::from_secs(0),
            var.interval,
            glib::clone!(@strong var => move |_| {
                let result: Result<_> = try {
                    evt_send.send(app::EwwCommand::UpdateVars(vec![(var.name.clone(), var.run_once()?)]))?;
                };
                util::print_result_err("while running script-var command", &result);
            }),
        );
        self.poll_handles.insert(var.name.clone(), handle);
    }

    /// initialize the tail_var handler thread
    pub fn setup_tail_tasks(&mut self) -> Result<()> {
        log::info!("initializing handler for tail script vars");

        let mut events = popol::Events::<VarName>::new();
        let evt_send = self.evt_send.clone();

        // TODO all of this is rather ugly
        let script_var_processes = self.tail_process_handles.clone();
        let sources = self.tail_sources.clone();
        let thread_handle = stoppable_thread::spawn(move |stopped| {
            while !stopped.get() {
                let result: Result<_> = try {
                    {
                        let _ = sources
                            .write()
                            .unwrap()
                            .wait_timeout(&mut events, std::time::Duration::from_millis(50));
                    }
                    for (var_name, event) in events.iter() {
                        if event.readable {
                            let mut handle = script_var_processes
                                .get_mut(var_name)
                                .with_context(|| format!("No command output handle found for variable '{}'", var_name))?;
                            let mut buffer = String::new();
                            handle.stdout_reader.read_line(&mut buffer)?;
                            evt_send.send(EwwCommand::UpdateVars(vec![(
                                var_name.to_owned(),
                                PrimitiveValue::from_string(buffer.trim_matches('\n').to_owned()),
                            )]))?;
                        } else if event.hangup {
                            script_var_processes.remove(var_name);
                            sources.write().unwrap().unregister(var_name);
                        }
                    }
                };
                util::print_result_err("in script-var tail handler thread", &result);
            }
            for process in script_var_processes.iter() {
                util::print_result_err("While killing tail-var process at the end of tail task", &process.kill());
            }
            script_var_processes.clear();
        });
        self.tail_handler_thread = Some(thread_handle);
        Ok(())
    }

    pub fn start_tail_script(&mut self, var: &config::TailScriptVar) {
        match ScriptVarProcess::run(&var.command) {
            Ok(process) => {
                self.tail_sources.write().unwrap().register(
                    var.name.clone(),
                    process.stdout_reader.get_ref(),
                    popol::interest::READ,
                );
                self.tail_process_handles.insert(var.name.clone(), process);
            }
            Err(err) => eprintln!("Failed to launch script-var command for tail: {:?}", err),
        }
    }
}

impl Drop for ScriptVarHandler {
    fn drop(&mut self) {
        self.stop();
    }
}

pub mod script_var_process {
    use anyhow::*;
    use nix::{
        sys::{signal, wait},
        unistd::Pid,
    };
    use std::{ffi::CString, io::BufReader, sync::Mutex};

    use crate::util;

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
            util::print_result_err("While killing process '{}' during cleanup", &result);
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
