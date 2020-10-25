use std::process::Stdio;

use crate::opts::{self, ActionClientOnly};
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
