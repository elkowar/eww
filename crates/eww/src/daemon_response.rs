use anyhow::*;

/// Response that the app may send as a response to a event.
/// This is used in `DaemonCommand`s that contain a response sender.
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_more::Display)]
pub enum DaemonResponse {
    Success(String),
    Failure(String),
}

impl DaemonResponse {
    pub fn is_success(&self) -> bool {
        matches!(self, DaemonResponse::Success(_))
    }

    pub fn is_failure(&self) -> bool {
        !self.is_success()
    }
}

#[derive(Debug)]
pub struct DaemonResponseSender(tokio::sync::mpsc::UnboundedSender<DaemonResponse>);

pub fn create_pair() -> (DaemonResponseSender, tokio::sync::mpsc::UnboundedReceiver<DaemonResponse>) {
    let (sender, recv) = tokio::sync::mpsc::unbounded_channel();
    (DaemonResponseSender(sender), recv)
}

impl DaemonResponseSender {
    pub fn send_success(&self, s: String) -> Result<()> {
        self.0.send(DaemonResponse::Success(s)).context("Failed to send success response from application thread")
    }

    pub fn send_failure(&self, s: String) -> Result<()> {
        self.0.send(DaemonResponse::Failure(s)).context("Failed to send failure response from application thread")
    }
}

pub type DaemonResponseReceiver = tokio::sync::mpsc::UnboundedReceiver<DaemonResponse>;
