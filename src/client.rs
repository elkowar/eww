use crate::{
    opts::{self, ActionClientOnly},
    util::{config_path, input, launch_editor, parse_scss_from_file},
};
use anyhow::*;
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    process::Stdio,
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
            // This is so ugly because of this: https://github.com/rust-lang/rfcs/issues/372
            let paths = config_path()?;
            let xml_file: std::path::PathBuf = paths.0;
            let scss_file: std::path::PathBuf = paths.1;
            let path: std::path::PathBuf;
            let file = file.unwrap_or_default();
            if file == "xml" {
                path = xml_file;
            } else if file == "scss" {
                path = scss_file;
            } else {
                eprint!("Edit the eww.xml file or the scss file? (X/s) ");
                path = if input()?.to_lowercase() == "s\n" {
                    scss_file
                } else {
                    xml_file
                }
            }

            launch_editor(&editor, path.to_str().unwrap())?;
            if path.extension().unwrap() == "xml" {
                while let Some(config) = crate::config::EwwConfig::read_from_file(&path).err() {
                    eprintln!("{}", config);
                    eprint!("The config file contains errors, edit again? (Y/n) ");
                    // \n is there because input is unsanitized and it still contains the newline
                    if input()?.to_lowercase() == "n\n" {
                        break;
                    } else {
                        launch_editor(&editor, path.to_str().unwrap())?;
                    };
                }
            } else {
                // I know these two while loops are ugly.. but functions don't really work because i couldn't use `break` and macros are wacky
                // And those 9 lines don't make a difference
                while let Some(config) = parse_scss_from_file(&path).err() {
                    eprintln!("{}", config);
                    eprint!("The config file contains errors, edit again? (Y/n) ");
                    let input = input()?;
                    // \n is there because input is unsanitized and it still contains the newline
                    if input.to_lowercase() == "n\n" {
                        break;
                    } else {
                        launch_editor(&editor, path.to_str().unwrap())?;
                    };
                }
            }
        }
    }
    Ok(())
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
