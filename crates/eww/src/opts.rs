use anyhow::*;
use eww_shared_util::VarName;
use serde::{Deserialize, Serialize};
use simplexpr::dynval::DynVal;
use structopt::StructOpt;
use yuck::{config::window_geometry::AnchorPoint, value::Coords};

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

#[derive(StructOpt, Debug, Serialize, Deserialize, PartialEq)]
struct RawOpt {
    /// Write out debug logs. (To read the logs, run `eww logs`).
    #[structopt(long = "debug", global = true)]
    log_debug: bool,

    /// override path to configuration directory (directory that contains eww.yuck and eww.scss)
    #[structopt(short, long, global = true)]
    config: Option<std::path::PathBuf>,

    /// Watch the log output after executing the command
    #[structopt(long = "logs", global = true)]
    show_logs: bool,

    /// Avoid daemonizing eww.
    #[structopt(long = "no-daemonize", global = true)]
    no_daemonize: bool,

    /// Restart the daemon completely before running the command
    #[structopt(long = "restart", global = true)]
    restart: bool,

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
        mappings: Vec<(VarName, DynVal)>,
    },

    /// Open the GTK debugger
    #[structopt(name = "inspector", alias = "debugger")]
    OpenInspector,

    /// Open a window
    #[structopt(name = "open", alias = "o")]
    OpenWindow {
        /// Name of the window you want to open.
        window_name: String,

        /// Monitor-index the window should open on
        #[structopt(long)]
        screen: Option<i32>,

        /// The position of the window, where it should open.
        #[structopt(short, long)]
        pos: Option<Coords>,

        /// The size of the window to open
        #[structopt(short, long)]
        size: Option<Coords>,

        /// Sidepoint of the window, formatted like "top right"
        #[structopt(short, long)]
        anchor: Option<AnchorPoint>,

        /// If the window is already open, close it instead
        #[structopt(long = "toggle")]
        should_toggle: bool,
    },

    /// Open multiple windows at once.
    /// NOTE: This will in the future be part of eww open, and will then be removed.
    #[structopt(name = "open-many")]
    OpenMany {
        windows: Vec<String>,

        /// If a window is already open, close it instead
        #[structopt(long = "toggle")]
        should_toggle: bool,
    },

    /// Close the given windows
    #[structopt(name = "close", alias = "c")]
    CloseWindows { windows: Vec<String> },

    /// Reload the configuration
    #[structopt(name = "reload", alias = "r")]
    Reload,

    /// Kill the eww daemon
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

    /// Get the value of a variable if defined
    #[structopt(name = "get")]
    GetVar { name: String },

    /// Print the names of all configured windows. Windows with a * in front of them are currently opened.
    #[structopt(name = "windows")]
    ShowWindows,

    /// Print out the widget structure as seen by eww.
    ///
    /// This may be useful if you are facing issues with how eww is interpreting your configuration,
    /// and to provide additional context to the eww developers if you are filing a bug.
    #[structopt(name = "debug")]
    ShowDebug,

    /// Print out the scope graph structure in graphviz dot format.
    #[structopt(name = "graph")]
    ShowGraph,
}

impl Opt {
    pub fn from_env() -> Self {
        let raw: RawOpt = StructOpt::from_args();
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
