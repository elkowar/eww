use std::process::Stdio;

use crate::{
    daemon_response::DaemonResponse,
    opts::{self, ActionClientOnly},
    EwwPaths,
};
use anyhow::*;
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
};

pub fn handle_client_only_action(paths: &EwwPaths, action: ActionClientOnly) -> Result<()> {
    match action {
        ActionClientOnly::Logs => {
            std::process::Command::new("tail")
                .args(["-f", paths.get_log_file().to_string_lossy().as_ref()].iter())
                .stdin(Stdio::null())
                .spawn()?
                .wait()?;
        }
    }
    Ok(())
}

/// Connect to the daemon and send the given request.
/// Returns the response from the daemon, or None if the daemon did not provide any useful response. An Ok(None) response does _not_ indicate failure.
pub fn do_server_call(stream: &mut UnixStream, action: &opts::ActionWithServer) -> Result<Option<DaemonResponse>> {
    log::debug!("Forwarding options to server");
    stream.set_nonblocking(false).context("Failed to set stream to non-blocking")?;

    let message_bytes = bincode::serialize(&action)?;

    stream.write(&(message_bytes.len() as u32).to_be_bytes()).context("Failed to send command size header to IPC stream")?;

    stream.write_all(&message_bytes).context("Failed to write command to IPC stream")?;

    let mut buf = Vec::new();
    stream.set_read_timeout(Some(std::time::Duration::from_millis(100))).context("Failed to set read timeout")?;
    stream.read_to_end(&mut buf).context("Error reading response from server")?;

    Ok(if buf.is_empty() {
        None
    } else {
        let buf = bincode::deserialize(&buf)?;
        Some(buf)
    })
}
