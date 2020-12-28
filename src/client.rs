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
            let (xml_file, scss_file) = config_path()?;
            let xml = crate::config::EwwConfig::read_from_file(&xml_file);

            let file = file.as_ref().map(|x| x.as_str());
            let path = match file {
                Some("xml") => xml_file,
                Some("scss") => scss_file,
                None => {
                    let mut string = String::from("Edit the eww.xml file or the scss file? (X/s) ");
                    let mut xmls: Vec<std::path::PathBuf> = Vec::new();
                    if xml.is_err() {
                        eprintln!(
                            "Main xml file contains errors, can't find out which other files there are.. \n{:?}\n",
                            xml.err().unwrap()
                        );
                    } else {
                        xmls.append(&mut xml.unwrap().included);
                        string = "eww.xml (X)\neww.scss (s)\n".to_string();
                        let mut a = 1;
                        for i in &xmls {
                            string.push_str(format!("{} (x{})\n", i.file_name().unwrap().to_str().unwrap(), a).as_str());
                            a += 1;
                        }
                        string.push_str("Which one of those do you want to edit? ")
                    }

                    eprint!("{}", string);
                    let input = input()?.to_lowercase().replace("\n", "");
                    if input == "s" {
                        scss_file
                    } else {
                        if input == "x" || input == "" {
                            xml_file
                        } else {
                            (&xmls[(input.get(1..).unwrap().parse::<usize>().unwrap()) - 1]).to_path_buf()
                        }
                    }
                }
                _ => std::path::PathBuf::new(),
            };
            fn_editor(&editor, &path)?;
        }
    }
    Ok(())
}
fn fn_editor(editor: &String, path: &std::path::Path) -> Result<()> {
    launch_editor(&editor, path.to_str().unwrap())?;
    let err = if path.extension().unwrap() == "xml" {
        crate::config::EwwConfig::read_from_file(&path).err()
    } else {
        parse_scss_from_file(&path).err()
    };
    match err {
        Some(_) => {
            eprintln!("{:?}", err.unwrap());
            eprint!("The config file contains errors, edit again? (Y/n) ");
            // \n is there because input is unsanitized and it still contains the newline
            if input()?.to_lowercase() != "n\n" {
                fn_editor(&editor, path)?;
            }
        }
        _ => {}
    };
    Ok(())
}

pub fn forward_command_to_server(mut stream: UnixStream, action: opts::ActionWithServer) -> Result<()> {
    log::info!("Forwarding options to server");
    stream.set_nonblocking(false)?;
    stream
        .write_all(&bincode::serialize(&action)?)
        .context("Failed to write command to IPC stream")?;

    let mut buf = String::new();
    stream.set_read_timeout(Some(std::time::Duration::from_millis(100)))?;
    stream.read_to_string(&mut buf)?;
    println!("{}", buf);
    Ok(())
}
