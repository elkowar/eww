use std::process::Command;

pub mod build_widget;
pub mod circular_progressbar;
pub mod def_widget_macro;
pub mod graph;
pub mod transform;
pub mod widget_definitions;

/// Run a command that was provided as an attribute.
/// This command may use placeholders which will be replaced by the values of the arguments given.
/// This can either be the placeholder `{}`, which will be replaced by the first argument,
/// Or a placeholder like `{0}`, `{1}`, etc, which will refer to the respective argument.
pub(self) fn run_command<T>(timeout: std::time::Duration, cmd: &str, args: &[T])
where
    T: 'static + std::fmt::Display + Send + Sync + Clone,
{
    use wait_timeout::ChildExt;
    let args = args.to_vec();
    let cmd = cmd.to_string();
    std::thread::spawn(move || {
        let cmd = if !args.is_empty() {
            args.iter()
                .enumerate()
                .fold(cmd.to_string(), |acc, (i, arg)| acc.replace(&format!("{{{}}}", i), &format!("{}", arg)))
                .replace("{{}}", &format!("{}", args[0]))
        } else {
            cmd
        };
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
