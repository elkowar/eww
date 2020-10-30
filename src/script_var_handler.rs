use std::{collections::HashMap, time::Duration};

use crate::{app, config, util, value::PrimitiveValue};
use anyhow::*;
use app::EwwCommand;
use glib;
use itertools::Itertools;
use scheduled_executor;
use std::io::BufRead;

use self::script_var_process::ScriptVarProcess;

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
        log::info!("Finished initializing script-var-handler");
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
                            evt_send.send(app::EwwCommand::UpdateVars(vec![(var.name.clone(), var.run_once()?)]))?;
                        };
                        util::print_result_err("while running script-var command", &result);
                    }),
                )
            })
            .collect_vec();
        log::info!("finished setting up poll tasks");
        Ok(())
    }

    /// initialize the tail_var handler thread
    pub fn setup_tail_tasks(&mut self, tail_script_vars: &[config::TailScriptVar]) -> Result<()> {
        log::info!("initializing handler for tail script vars");
        let mut sources = popol::Sources::with_capacity(tail_script_vars.len());

        let mut script_var_processes: HashMap<_, ScriptVarProcess> = HashMap::new();

        for var in tail_script_vars {
            match ScriptVarProcess::run(&var.command) {
                Ok(process) => {
                    sources.register(var.name.clone(), process.stdout_reader.get_ref(), popol::interest::READ);
                    script_var_processes.insert(var.name.clone(), process);
                }
                Err(err) => eprintln!("Failed to launch script-var command for tail: {:?}", err),
            }
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
                            let handle = script_var_processes
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
                            sources.unregister(var_name);
                        }
                    }
                };
                util::print_result_err("in script-var tail handler thread", &result);
            }
            for process in script_var_processes.values() {
                util::print_result_err("While killing tail-var process at the end of tail task", &process.kill());
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
        pid: i32,
        pub stdout_reader: BufReader<filedescriptor::FileDescriptor>,
    }

    impl ScriptVarProcess {
        pub(super) fn run(command: &str) -> Result<Self> {
            use nix::unistd::*;

            let pipe = filedescriptor::Pipe::new()?;

            match unsafe { fork()? } {
                ForkResult::Parent { child, .. } => {
                    SCRIPT_VAR_CHILDREN.lock().unwrap().push(child.as_raw() as u32);

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
