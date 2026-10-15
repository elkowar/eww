use std::process::Stdio;

use crate::{
    daemon_response::DaemonResponse,
    ipc_server,
    opts::{self, ActionClientOnly},
    paths::EwwPaths,
};
use anyhow::{Context, Result};
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
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

    // A ping is answered by the IPC server itself. Other commands wait for the app, so outlast the daemon's timeout and let it
    // close the connection, instead of failing here and making `eww open` start a second daemon.
    let read_timeout = match action {
        opts::ActionWithServer::Ping => Duration::from_millis(100),
        _ => ipc_server::RESPONSE_TIMEOUT * 2,
    };

    let mut buf = Vec::new();
    stream.set_read_timeout(Some(read_timeout)).context("Failed to set read timeout")?;
    stream.read_to_end(&mut buf).context("Error reading response from server")?;

    Ok(if buf.is_empty() {
        None
    } else {
        let buf = bincode::deserialize(&buf)?;
        Some(buf)
    })
}
