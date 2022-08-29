//! Types to manage messages that notify the eww client over the result of a command
//!
//! Communcation between the daemon and eww client happens via IPC.
//! If the daemon needs to send messages back to the client as a response to a command (mostly for CLI output),
//! this happens via the DaemonResponse types

use anyhow::{Context, Result};
use itertools::Itertools;
use tokio::sync::mpsc;

use crate::error_handling_ctx;

/// Response that the app may send as a response to a event.
/// This is used in `DaemonCommand`s that contain a response sender.
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_more::Display)]
pub enum DaemonResponse {
    Success(String),
    Failure(String),
}

#[derive(Debug)]
pub struct DaemonResponseSender(mpsc::UnboundedSender<DaemonResponse>);

pub fn create_pair() -> (DaemonResponseSender, mpsc::UnboundedReceiver<DaemonResponse>) {
    let (sender, recv) = mpsc::unbounded_channel();
    (DaemonResponseSender(sender), recv)
}

impl DaemonResponseSender {
    pub fn send_success(&self, s: String) -> Result<()> {
        self.0.send(DaemonResponse::Success(s)).context("Failed to send success response from application thread")
    }

    pub fn send_failure(&self, s: String) -> Result<()> {
        self.0.send(DaemonResponse::Failure(s)).context("Failed to send failure response from application thread")
    }

    /// Given a list of errors, respond with an error value if there are any errors, and respond with success otherwise.
    pub fn respond_with_error_list(&self, errors: impl IntoIterator<Item = anyhow::Error>) -> Result<()> {
        let errors = errors.into_iter().map(|e| error_handling_ctx::format_error(&e)).join("\n");
        if errors.is_empty() {
            self.send_success(String::new())
        } else {
            self.respond_with_error_msg(errors)
        }
    }

    /// In case of an Err, send the error message to a sender.
    pub fn respond_with_result<T>(&self, result: Result<T>) -> Result<()> {
        match result {
            Ok(_) => self.send_success(String::new()),
            Err(e) => {
                let formatted = error_handling_ctx::format_error(&e);
                self.respond_with_error_msg(formatted)
            }
        }
        .context("sending response from main thread")
    }

    fn respond_with_error_msg(&self, msg: String) -> Result<()> {
        println!("Action failed with error: {}", msg);
        self.send_failure(msg)
    }
}

pub type DaemonResponseReceiver = mpsc::UnboundedReceiver<DaemonResponse>;
