use std::process::Stdio;

use crate::{
    app,
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
    }
    Ok(())
}

pub fn do_server_call(mut stream: UnixStream, action: opts::ActionWithServer) -> Result<Option<app::DaemonResponse>> {
    log::info!("Forwarding options to server");
    stream
        .set_nonblocking(false)
        .context("Failed to set stream to non-blocking")?;

    let message_bytes = bincode::serialize(&action)?;

    stream
        .write(&(message_bytes.len() as u32).to_be_bytes())
        .context("Failed to send command size header to IPC stream")?;

    stream
        .write_all(&message_bytes)
        .context("Failed to write command to IPC stream")?;

    let mut buf = Vec::new();
    stream
        .set_read_timeout(Some(std::time::Duration::from_millis(1000)))
        .context("Failed to set read timeout")?;
    stream.read_to_end(&mut buf).context("Error reading response from server")?;

    Ok(if buf.is_empty() {
        None
    } else {
        let buf = bincode::deserialize(&buf)?;
        Some(buf)
    })
}
