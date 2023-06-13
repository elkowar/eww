use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Dbus connection error")]
    DbusError(#[from] zbus::Error),
    #[error("Service path {0} was not understood")]
    DbusAddressError(String),
}

pub type Result<T> = std::result::Result<T, Error>;
