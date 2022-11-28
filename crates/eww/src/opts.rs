use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use eww_shared_util::VarName;
use serde::{Deserialize, Serialize};
use simplexpr::dynval::DynVal;
use yuck::{
    config::{monitor::MonitorIdentifier, window_geometry::AnchorPoint},
    value::Coords,
};

use crate::{
    app,
    daemon_response::{self, DaemonResponse, DaemonResponseSender},
};

/// Struct that gets generated from `RawOpt`.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Opt {
    pub log_debug: bool,
    pub show_logs: bool,
    pub restart: bool,
    pub config_path: Option<std::path::PathBuf>,
    pub action: Action,
    pub no_daemonize: bool,
}

#[derive(Parser, Debug, Serialize, Deserialize, PartialEq)]
#[clap(author = "ElKowar")]
#[clap(version, about)]
struct RawOpt {
    /// Write out debug logs. (To read the logs, run `eww logs`).
    #[arg(long = "debug", global = true)]
    log_debug: bool,

    /// override path to configuration directory (directory that contains eww.yuck and eww.scss)
    #[arg(short, long, global = true)]
    config: Option<std::path::PathBuf>,

    /// Watch the log output after executing the command
    #[arg(long = "logs", global = true)]
    show_logs: bool,

    /// Avoid daemonizing eww.
    #[arg(long = "no-daemonize", global = true)]
    no_daemonize: bool,

    /// Restart the daemon completely before running the command
    #[arg(long = "restart", global = true)]
    restart: bool,

    #[command(subcommand)]
    action: Action,
}

#[derive(Subcommand, Debug, Serialize, Deserialize, PartialEq)]
pub enum Action {
    /// Start the Eww daemon.
    #[command(name = "daemon", alias = "d")]
    Daemon,

    #[command(flatten)]
    ClientOnly(ActionClientOnly),

    #[command(flatten)]
    WithServer(ActionWithServer),
}

#[derive(Subcommand, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ActionClientOnly {
    /// Print and watch the eww logs
    #[command(name = "logs")]
    Logs,
}

#[derive(Subcommand, Debug, Serialize, Deserialize, PartialEq)]
pub enum ActionWithServer {
    /// Ping the eww server, checking if it is reachable.
    #[clap(name = "ping")]
    Ping,

    /// Update the value of a variable, in a running eww instance
    #[clap(name = "update", alias = "u")]
    Update {
        /// variable_name="new_value"-pairs that will be updated
        #[arg(value_parser = parse_var_update_arg)]
        mappings: Vec<(VarName, DynVal)>,
    },

    /// Open the GTK debugger
    #[command(name = "inspector", alias = "debugger")]
    OpenInspector,

    /// Open a window
    #[clap(name = "open", alias = "o")]
    OpenWindow {
        /// Name of the window you want to open.
        window_name: String,

        /// The identifier of the monitor the window should open on
        #[arg(long)]
        screen: Option<MonitorIdentifier>,

        /// The position of the window, where it should open. (i.e.: 200x100)
        #[arg(short, long)]
        pos: Option<Coords>,

        /// The size of the window to open (i.e.: 200x100)
        #[arg(short, long)]
        size: Option<Coords>,

        /// Sidepoint of the window, formatted like "top right"
        #[arg(short, long)]
        anchor: Option<AnchorPoint>,

        /// If the window is already open, close it instead
        #[arg(long = "toggle")]
        should_toggle: bool,
    },

    /// Open multiple windows at once.
    /// NOTE: This will in the future be part of eww open, and will then be removed.
    #[command(name = "open-many")]
    OpenMany {
        windows: Vec<String>,

        /// If a window is already open, close it instead
        #[arg(long = "toggle")]
        should_toggle: bool,
    },

    /// Close the given windows
    #[command(name = "close", alias = "c")]
    CloseWindows { windows: Vec<String> },

    /// Reload the configuration
    #[command(name = "reload", alias = "r")]
    Reload,

    /// Kill the eww daemon
    #[command(name = "kill", alias = "k")]
    KillServer,

    /// Close all windows, without killing the daemon
    #[command(name = "close-all", alias = "ca")]
    CloseAll,

    /// Prints the variables used in all currently open window
    #[command(name = "state")]
    ShowState {
        /// Shows all variables, including not currently used ones
        #[arg(short, long)]
        all: bool,
    },

    /// Get the value of a variable if defined
    #[command(name = "get")]
    GetVar { name: String },

    /// Print the names of all configured windows. Windows with a * in front of them are currently opened.
    #[command(name = "windows")]
    ShowWindows,

    /// Print out the widget structure as seen by eww.
    ///
    /// This may be useful if you are facing issues with how eww is interpreting your configuration,
    /// and to provide additional context to the eww developers if you are filing a bug.
    #[command(name = "debug")]
    ShowDebug,

    /// Print out the scope graph structure in graphviz dot format.
    #[command(name = "graph")]
    ShowGraph,
}

impl Opt {
    pub fn from_env() -> Self {
        let raw: RawOpt = RawOpt::parse();
        raw.into()
    }
}

impl From<RawOpt> for Opt {
    fn from(other: RawOpt) -> Self {
        let RawOpt { log_debug, config, show_logs, no_daemonize, restart, action } = other;
        Opt { log_debug, show_logs, restart, config_path: config, action, no_daemonize }
    }
}

fn parse_var_update_arg(s: &str) -> Result<(VarName, DynVal)> {
    let (name, value) = s
        .split_once('=')
        .with_context(|| format!("arguments must be in the shape `variable_name=\"new_value\"`, but got: {}", s))?;
    Ok((name.into(), DynVal::from_string(value.to_owned())))
}

impl ActionWithServer {
    pub fn can_start_daemon(&self) -> bool {
        matches!(self, ActionWithServer::OpenWindow { .. } | ActionWithServer::OpenMany { .. })
    }

    pub fn into_daemon_command(self) -> (app::DaemonCommand, Option<daemon_response::DaemonResponseReceiver>) {
        let command = match self {
            ActionWithServer::Update { mappings } => app::DaemonCommand::UpdateVars(mappings),
            ActionWithServer::OpenInspector => app::DaemonCommand::OpenInspector,

            ActionWithServer::KillServer => app::DaemonCommand::KillServer,
            ActionWithServer::CloseAll => app::DaemonCommand::CloseAll,
            ActionWithServer::Ping => {
                let (send, recv) = tokio::sync::mpsc::unbounded_channel();
                let _ = send.send(DaemonResponse::Success("pong".to_owned()));
                return (app::DaemonCommand::NoOp, Some(recv));
            }
            ActionWithServer::OpenMany { windows, should_toggle } => {
                return with_response_channel(|sender| app::DaemonCommand::OpenMany { windows, should_toggle, sender });
            }
            ActionWithServer::OpenWindow { window_name, pos, size, screen, anchor, should_toggle } => {
                return with_response_channel(|sender| app::DaemonCommand::OpenWindow {
                    window_name,
                    pos,
                    size,
                    anchor,
                    screen,
                    should_toggle,
                    sender,
                })
            }
            ActionWithServer::CloseWindows { windows } => {
                return with_response_channel(|sender| app::DaemonCommand::CloseWindows { windows, sender });
            }
            ActionWithServer::Reload => return with_response_channel(app::DaemonCommand::ReloadConfigAndCss),
            ActionWithServer::ShowWindows => return with_response_channel(app::DaemonCommand::PrintWindows),
            ActionWithServer::ShowState { all } => {
                return with_response_channel(|sender| app::DaemonCommand::PrintState { all, sender })
            }
            ActionWithServer::GetVar { name } => {
                return with_response_channel(|sender| app::DaemonCommand::GetVar { name, sender })
            }
            ActionWithServer::ShowDebug => return with_response_channel(app::DaemonCommand::PrintDebug),
            ActionWithServer::ShowGraph => return with_response_channel(app::DaemonCommand::PrintGraph),
        };
        (command, None)
    }
}

fn with_response_channel<O, F>(f: F) -> (O, Option<tokio::sync::mpsc::UnboundedReceiver<DaemonResponse>>)
where
    F: FnOnce(DaemonResponseSender) -> O,
{
    let (sender, recv) = daemon_response::create_pair();
    (f(sender), Some(recv))
}
