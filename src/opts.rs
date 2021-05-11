use anyhow::*;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use crate::{
    app,
    config::{AnchorPoint, WindowName},
    value::{Coords, PrimVal, VarName},
};

/// Struct that gets generated from `RawOpt`.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Opt {
    pub log_debug: bool,
    pub config_path: Option<std::path::PathBuf>,
    pub action: Action,
}

#[derive(StructOpt, Debug, Serialize, Deserialize, PartialEq)]
struct RawOpt {
    /// Write out debug logs. (To read the logs, run `eww logs`).
    #[structopt(long = "debug", global = true)]
    log_debug: bool,

    /// override path to configuration directory (directory that contains eww.xml and eww.scss)
    #[structopt(short, long, global = true)]
    config: Option<std::path::PathBuf>,

    #[structopt(subcommand)]
    action: Action,
}

#[derive(StructOpt, Debug, Serialize, Deserialize, PartialEq)]
pub enum Action {
    /// Start the Eww daemon.
    #[structopt(name = "daemon", alias = "d")]
    Daemon,

    #[structopt(flatten)]
    ClientOnly(ActionClientOnly),

    #[structopt(flatten)]
    WithServer(ActionWithServer),
}

#[derive(StructOpt, Debug, Serialize, Deserialize, PartialEq)]
pub enum ActionClientOnly {
    /// Print and watch the eww logs
    #[structopt(name = "logs")]
    Logs,
}

#[derive(StructOpt, Debug, Serialize, Deserialize, PartialEq)]
pub enum ActionWithServer {
    /// Ping the eww server, checking if it is reachable.
    #[structopt(name = "ping")]
    Ping,

    /// Update the value of a variable, in a running eww instance
    #[structopt(name = "update", alias = "u")]
    Update {
        /// variable_name="new_value"-pairs that will be updated
        #[structopt(parse(try_from_str = parse_var_update_arg))]
        mappings: Vec<(VarName, PrimVal)>,
    },

    /// open a window
    #[structopt(name = "open", alias = "o")]
    OpenWindow {
        /// Name of the window you want to open.
        window_name: WindowName,

        /// Monitor-index the window should open on
        #[structopt(short, long)]
        monitor: Option<i32>,

        /// The position of the window, where it should open.
        #[structopt(short, long)]
        pos: Option<Coords>,

        /// The size of the window to open
        #[structopt(short, long)]
        size: Option<Coords>,

        /// Sidepoint of the window, formatted like "top right"
        #[structopt(short, long)]
        anchor: Option<AnchorPoint>,
    },

    /// Open multiple windows at once.
    /// NOTE: This will in the future be part of eww open, and will then be removed.
    #[structopt(name = "open-many")]
    OpenMany { windows: Vec<WindowName> },

    /// Close the window with the given name
    #[structopt(name = "close", alias = "c")]
    CloseWindow { window_name: WindowName },

    /// Reload the configuration
    #[structopt(name = "reload", alias = "r")]
    Reload,

    /// kill the eww daemon
    #[structopt(name = "kill", alias = "k")]
    KillServer,

    /// Close all windows, without killing the daemon
    #[structopt(name = "close-all", alias = "ca")]
    CloseAll,

    /// Prints the variables used in all currently open window
    #[structopt(name = "state")]
    ShowState {
        /// Shows all variables, including not currently used ones
        #[structopt(short, long)]
        all: bool,
    },

    /// Print the names of all configured windows. Windows with a * in front of them are currently opened.
    #[structopt(name = "windows")]
    ShowWindows,

    /// Print out the widget structure as seen by eww.
    ///
    /// This may be useful if you are facing issues with how eww is interpreting your configuration,
    /// and to provide additional context to the eww developers if you are filing a bug.
    #[structopt(name = "debug")]
    ShowDebug,
}

impl Opt {
    pub fn from_env() -> Self {
        let raw: RawOpt = StructOpt::from_args();
        raw.into()
    }
}

impl From<RawOpt> for Opt {
    fn from(other: RawOpt) -> Self {
        let RawOpt { action, log_debug, config } = other;
        Opt { action, log_debug, config_path: config }
    }
}

fn parse_var_update_arg(s: &str) -> Result<(VarName, PrimVal)> {
    let (name, value) = s
        .split_once('=')
        .with_context(|| format!("arguments must be in the shape `variable_name=\"new_value\"`, but got: {}", s))?;
    Ok((name.into(), PrimVal::from_string(value.to_owned())))
}

impl ActionWithServer {
    pub fn into_daemon_command(self) -> (app::DaemonCommand, Option<app::DaemonResponseReceiver>) {
        let command = match self {
            ActionWithServer::Update { mappings } => app::DaemonCommand::UpdateVars(mappings.into_iter().collect()),

            ActionWithServer::KillServer => app::DaemonCommand::KillServer,
            ActionWithServer::CloseAll => app::DaemonCommand::CloseAll,
            ActionWithServer::Ping => {
                let (send, recv) = tokio::sync::mpsc::unbounded_channel();
                let _ = send.send(app::DaemonResponse::Success("pong".to_owned()));
                return (app::DaemonCommand::NoOp, Some(recv));
            }
            ActionWithServer::OpenMany { windows } => {
                return with_response_channel(|sender| app::DaemonCommand::OpenMany { windows, sender });
            }
            ActionWithServer::OpenWindow { window_name, pos, size, monitor, anchor } => {
                return with_response_channel(|sender| app::DaemonCommand::OpenWindow {
                    window_name,
                    pos,
                    size,
                    anchor,
                    monitor,
                    sender,
                })
            }
            ActionWithServer::CloseWindow { window_name } => {
                return with_response_channel(|sender| app::DaemonCommand::CloseWindow { window_name, sender });
            }
            ActionWithServer::Reload => return with_response_channel(app::DaemonCommand::ReloadConfigAndCss),
            ActionWithServer::ShowWindows => return with_response_channel(app::DaemonCommand::PrintWindows),
            ActionWithServer::ShowState { all } => {
                return with_response_channel(|sender| app::DaemonCommand::PrintState { all, sender })
            }
            ActionWithServer::ShowDebug => return with_response_channel(app::DaemonCommand::PrintDebug),
        };
        (command, None)
    }
}

fn with_response_channel<T, O, F>(f: F) -> (O, Option<tokio::sync::mpsc::UnboundedReceiver<T>>)
where
    F: FnOnce(tokio::sync::mpsc::UnboundedSender<T>) -> O,
{
    let (sender, recv) = tokio::sync::mpsc::unbounded_channel();
    (f(sender), Some(recv))
}
