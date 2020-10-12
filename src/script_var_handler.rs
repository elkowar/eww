use crate::{app, config, eww_state};
use anyhow::*;
use glib;
use itertools::Itertools;
use scheduled_executor;
pub struct ScriptVarHandler {
    evt_send: glib::Sender<app::EwwEvent>,
    pub poll_handles: Vec<scheduled_executor::executor::TaskHandle>,
    pub poll_executor: scheduled_executor::CoreExecutor,
}

impl ScriptVarHandler {
    pub fn new(evt_send: glib::Sender<app::EwwEvent>) -> Result<Self> {
        log::info!("initializing handler for poll script vars");
        Ok(ScriptVarHandler {
            evt_send,
            poll_handles: Vec::new(),
            poll_executor: scheduled_executor::CoreExecutor::new()?,
        })
    }

    /// clears and stops the currently running poll handles, then opens the new
    /// ones as configured
    pub fn setup_command_poll_tasks(&mut self, config: &config::EwwConfig) -> Result<()> {
        log::info!("reloading handler for poll script vars");
        self.poll_handles.iter().for_each(|handle| handle.stop());
        self.poll_handles.clear();

        let evt_send = self.evt_send.clone();
        self.poll_handles = config
            .get_script_vars()
            .iter()
            .map(|var| {
                self.poll_executor.schedule_fixed_interval(
                    std::time::Duration::from_secs(0),
                    var.interval,
                    glib::clone!(@strong var, @strong evt_send => move |_| {
                        let result = eww_state::run_command(&var.command)
                            .and_then(|output| Ok(evt_send.send(app::EwwEvent::UpdateVar(var.name.clone(), output))?));
                        if let Err(e) = result {
                            eprintln!("Error while running script-var command: {:?}", e);
                        }
                    }),
                )
            })
            .collect_vec();
        Ok(())
    }
}
