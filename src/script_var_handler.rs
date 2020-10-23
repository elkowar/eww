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
                            evt_send.send(app::EwwCommand::UpdateVar(var.name.clone(), var.run_once()?))?;
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
                            evt_send.send(EwwCommand::UpdateVar(
                                var_name.to_owned(),
                                PrimitiveValue::from_string(buffer.trim_matches('\n').to_owned()),
                            ))?;
                        } else if event.hangup {
                            script_var_processes.remove(var_name);
                            sources.unregister(var_name);
                        }
                    }
                };
                util::print_result_err("in script-var tail handler thread", &result);
            }
            script_var_processes.values().for_each(|process| process.kill());
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
    use std::{io::BufReader, process::Stdio, sync::Mutex};

    use crate::util;

    lazy_static::lazy_static! {
        static ref SCRIPT_VAR_CHILDREN: Mutex<Vec<u32>> = Mutex::new(Vec::new());
    }

    fn terminate_pid(pid: u32) {
        println!("Killing pid: {}", pid);
        let result = signal::kill(Pid::from_raw(pid as i32), signal::SIGTERM);
        util::print_result_err("While killing tail-var child processes", &result);
        let wait_result = wait::waitpid(Pid::from_raw(pid as i32), None);
        util::print_result_err("While killing tail-var child processes", &wait_result);
    }

    /// This function should be called in the signal handler, killing all child processes.
    pub fn on_application_death() {
        SCRIPT_VAR_CHILDREN
            .lock()
            .unwrap()
            .drain(..)
            .for_each(|pid| terminate_pid(pid));
    }

    pub struct ScriptVarProcess {
        child: std::process::Child,
        pub stdout_reader: BufReader<std::process::ChildStdout>,
    }

    impl ScriptVarProcess {
        pub(super) fn run(command: &str) -> Result<Self> {
            println!("Running {}", command);
            let mut child = std::process::Command::new("/bin/sh")
                .arg("-c")
                .arg(command)
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .stdin(Stdio::null())
                .spawn()?;
            SCRIPT_VAR_CHILDREN.lock().unwrap().push(child.id());
            Ok(ScriptVarProcess {
                stdout_reader: BufReader::new(child.stdout.take().unwrap()),
                child,
            })
        }

        pub(super) fn kill(&self) {
            SCRIPT_VAR_CHILDREN.lock().unwrap().retain(|item| *item != self.child.id());
            terminate_pid(self.child.id());
        }
    }

    impl Drop for ScriptVarProcess {
        fn drop(&mut self) {
            self.kill();
        }
    }
}
