use std::process::Stdio;

use crate::{
    config,
    opts::{self, ActionClientOnly},
};
use anyhow::*;
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
};

pub fn handle_client_only_action(action: ActionClientOnly) -> Result<()> {
    match action {
        ActionClientOnly::Logs => {
            std::process::Command::new("tail")
                .args(["-f", crate::LOG_FILE.to_string_lossy().as_ref()].iter())
                .stdin(Stdio::null())
                .spawn()?
                .wait()?;
        }
        ActionClientOnly::Edit { editor_arg, file } => {
            // what editor to use
            let editor_err = std::env::var("EDITOR");
            let editor: String;
            if editor_err.is_err() && editor_arg.is_none() {
                eprintln!("EDITOR environment variable not set. Try running with -e '<editor>' or set the environment variable");
                std::process::exit(1);
            } else if editor_arg.is_some() {
                editor = editor_arg.unwrap();
            } else {
                editor = editor_err?;
            }
            // what file to edit, the xml or the scss file
            let paths = crate::util::config_path()?;
            let xml_file: std::path::PathBuf = paths.0;
            let scss_file: std::path::PathBuf = paths.1;
            let path: std::path::PathBuf;
            let file = file.unwrap_or_default();
            if file == "xml" {
                path = xml_file;
            } else if file == "scss" {
                path = scss_file;
            } else {
                eprint!("Edit the eww.xml file (if no it's the eww.scss) (Y/n) ");
                let input = input()?;
                path = if input.to_lowercase() == "n\n" { scss_file } else { xml_file }
            }
            // have to do this so that the EDITOR environment variable,
            // gets parsed as one and not as several args,
            // so that e.g. your EDITOR env variable is equal to `vim -xx`
            // then that gets started as such and not as the binary `vim -xx`
            // If we'd split the issue could be that the space was originally escaped but not here because it would be removed from the shell
            // tho that wouldn't happen with env variables
            launch_editor(&editor, path.to_str().unwrap())?;
            let config = config::EwwConfig::read_from_file(&path).err();
            while config.is_some() {
                eprintln!("{}", config.as_ref().unwrap());
                eprint!("The config file contains errors, edit again? (Y/n) ");
                let input = input()?;
                if input.to_lowercase() == "n\n" {
                    break;
                } else {
                    launch_editor(&editor, path.to_str().unwrap())?;
                };
            }
        }
    }
    Ok(())
}
fn input() -> Result<String> {
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input)
}
fn launch_editor(editor: &String, path: &str) -> Result<std::process::ExitStatus> {
    Ok(std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{} {}", editor, path))
        .spawn()?
        .wait()?)
}

pub fn forward_command_to_server(mut stream: UnixStream, action: opts::ActionWithServer) -> Result<()> {
    log::info!("Forwarding options to server");
    stream.write_all(&bincode::serialize(&action)?)?;

    let mut buf = String::new();
    stream.set_read_timeout(Some(std::time::Duration::from_millis(100)))?;
    stream.read_to_string(&mut buf)?;
    println!("{}", buf);
    Ok(())
}
