//! Module concerned with handling the global application lifecycle of eww.
//! Currently, this only means handling application exit by providing a global
//! `recv_exit()` function which can be awaited to receive an event in case of application termination.

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use tokio::sync::broadcast;

pub static APPLICATION_EXIT_SENDER: Lazy<broadcast::Sender<()>> = Lazy::new(|| broadcast::channel(2).0);

/// Notify all listening tasks of the termination of the eww application process.
pub fn send_exit() -> Result<()> {
    (APPLICATION_EXIT_SENDER).send(()).context("Failed to send exit lifecycle event")?;
    Ok(())
}

/// Yields Ok(()) on application termination. Await on this in all long-running tasks
/// and perform any cleanup if necessary.
pub async fn recv_exit() -> Result<()> {
    (APPLICATION_EXIT_SENDER).subscribe().recv().await.context("Failed to receive lifecycle event")
}

/// Select in a loop, breaking once a application termination event (see `crate::application_lifecycle`) is received.
#[macro_export]
macro_rules! loop_select_exiting {
    ($($content:tt)*) => {
        loop {
            tokio::select! {
                Ok(()) = $crate::application_lifecycle::recv_exit() => {
                    break;
                }
                $($content)*
            }
        }
    };
}
