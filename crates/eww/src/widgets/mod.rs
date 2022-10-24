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
    let cmd = replace_placeholders(cmd, args);
    std::thread::Builder::new()
        .name("command-execution-thread".to_string())
        .spawn(move || {
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
        })
        .expect("Failed to start command-execution-thread");
}

fn replace_placeholders<T>(cmd: &str, args: &[T]) -> String
where
    T: 'static + std::fmt::Display + Send + Sync + Clone,
{
    if !args.is_empty() {
        let cmd = cmd.replace("{}", &format!("{}", args[0]));
        args.iter().enumerate().fold(cmd, |acc, (i, arg)| acc.replace(&format!("{{{}}}", i), &format!("{}", arg)))
    } else {
        cmd.to_string()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_replace_placeholders() {
        assert_eq!("foo", replace_placeholders("foo", &[""]),);
        assert_eq!("foo hi", replace_placeholders("foo {}", &["hi"]),);
        assert_eq!("foo hi", replace_placeholders("foo {}", &["hi", "ho"]),);
        assert_eq!("bar foo baz", replace_placeholders("{0} foo {1}", &["bar", "baz"]),);
        assert_eq!("baz foo bar", replace_placeholders("{1} foo {0}", &["bar", "baz"]),);
    }
}
