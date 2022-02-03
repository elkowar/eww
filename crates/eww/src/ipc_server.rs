use crate::{app, opts};
use anyhow::{Context, Result};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc::*,
};

pub async fn run_server<P: AsRef<std::path::Path>>(evt_send: UnboundedSender<app::DaemonCommand>, socket_path: P) -> Result<()> {
    let socket_path = socket_path.as_ref();
    let listener = { tokio::net::UnixListener::bind(socket_path)? };
    log::info!("IPC server initialized");
    crate::loop_select_exiting! {
        connection = listener.accept() => match connection {
            Ok((stream, _addr)) => {
                let evt_send = evt_send.clone();
                tokio::spawn(async move {
                    let result = handle_connection(stream, evt_send.clone()).await;
                    crate::print_result_err!("while handling IPC connection with client", result);
                });
            },
            Err(e) => eprintln!("Failed to connect to client: {:?}", e),
        }
    }
    Ok(())
}

/// Handle a single IPC connection from start to end.
async fn handle_connection(mut stream: tokio::net::UnixStream, evt_send: UnboundedSender<app::DaemonCommand>) -> Result<()> {
    let (mut stream_read, mut stream_write) = stream.split();

    let action: opts::ActionWithServer = read_action_from_stream(&mut stream_read).await?;

    log::debug!("received command from IPC: {:?}", &action);

    let (command, maybe_response_recv) = action.into_daemon_command();

    evt_send.send(command)?;

    if let Some(mut response_recv) = maybe_response_recv {
        log::debug!("Waiting for response for IPC client");
        if let Ok(Some(response)) = tokio::time::timeout(Duration::from_millis(100), response_recv.recv()).await {
            let response = bincode::serialize(&response)?;
            let result = &stream_write.write_all(&response).await;
            crate::print_result_err!("sending text response to ipc client", &result);
        }
    }
    stream_write.shutdown().await?;
    Ok(())
}

/// Read a single message from a unix stream, and parses it into a `ActionWithServer`
/// The format here requires the first 4 bytes to be the size of the rest of the message (in big-endian), followed by the rest of the message.
async fn read_action_from_stream(stream_read: &'_ mut tokio::net::unix::ReadHalf<'_>) -> Result<opts::ActionWithServer> {
    let mut message_byte_length = [0u8; 4];
    stream_read.read_exact(&mut message_byte_length).await.context("Failed to read message size header in IPC message")?;
    let message_byte_length = u32::from_be_bytes(message_byte_length);
    let mut raw_message = Vec::<u8>::with_capacity(message_byte_length as usize);
    while raw_message.len() < message_byte_length as usize {
        stream_read.read_buf(&mut raw_message).await.context("Failed to read actual IPC message")?;
    }

    bincode::deserialize(&raw_message).context("Failed to parse client message")
}
