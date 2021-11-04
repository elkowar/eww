use std::process::Command;

pub mod build_widget;
pub mod widget_definitions;
pub mod def_widget_macro;

const CMD_STRING_PLACEHODLER: &str = "{}";

/// Run a command that was provided as an attribute. This command may use a
/// placeholder ('{}') which will be replaced by the value provided as [`arg`]
pub(self) fn run_command<T: 'static + std::fmt::Display + Send + Sync>(timeout: std::time::Duration, cmd: &str, arg: T) {
    use wait_timeout::ChildExt;
    let cmd = cmd.to_string();
    std::thread::spawn(move || {
        let cmd = cmd.replace(CMD_STRING_PLACEHODLER, &format!("{}", arg));
        log::debug!("Running command from widget: {}", cmd);
        let child = Command::new("/bin/sh").arg("-c").arg(&cmd).spawn();
        match child {
            Ok(mut child) => match child.wait_timeout(timeout) {
                // child timed out
                Ok(None) => {
                    log::error!("WARNING: command {} timed out", &cmd);
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
