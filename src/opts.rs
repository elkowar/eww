use anyhow::*;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use crate::{
    app,
    config::{AnchorPoint, WindowName},
    value::{Coords, PrimitiveValue, VarName},
};

#[derive(StructOpt, Debug, Serialize, Deserialize, PartialEq)]
pub struct Opt {
    #[structopt(subcommand)]
    pub action: Action,

    /// Run Eww in the background, daemonizing it.
    /// When daemonized, to kill eww you can run `eww kill`. To see logs, use `eww logs`.
    #[structopt(short = "-d", long = "--detach")]
    pub should_detach: bool,
}

#[derive(StructOpt, Debug, Serialize, Deserialize, PartialEq)]
pub enum Action {
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
    /// Edit the configuration files
    #[structopt(name = "edit")]
    Edit {
        /// Editor to use
        #[structopt(name = "editor", short, long)]
        editor_arg: Option<String>,
        /// What file to edit (the main xml file or the main scss file) accepted values are xml or scss
        #[structopt(name = "file", short, long)]
        file: Option<String>,
    },
}

#[derive(StructOpt, Debug, Serialize, Deserialize, PartialEq)]
pub enum ActionWithServer {
    /// Update the value of a variable, in a running eww instance
    #[structopt(name = "update")]
    Update {
        /// variable_name="new_value"-pairs that will be updated
        #[structopt(parse(try_from_str = parse_var_update_arg))]
        mappings: Vec<(VarName, PrimitiveValue)>,
    },

    /// open a window
    #[structopt(name = "open")]
    OpenWindow {
        /// Name of the window you want to open.
        window_name: WindowName,

        /// The position of the window, where it should open.
        #[structopt(short, long)]
        pos: Option<Coords>,

        /// The size of the window to open
        #[structopt(short, long)]
        size: Option<Coords>,

        /// Anchorpoint of the window, formatted like "top right"
        #[structopt(short, long)]
        anchor: Option<AnchorPoint>,
    },

    /// Close the window with the given name
    #[structopt(name = "close")]
    CloseWindow { window_name: WindowName },

    /// kill the eww daemon
    #[structopt(name = "kill")]
    KillServer,

    /// Print the current eww-state
    #[structopt(name = "state")]
    ShowState,

    /// Print out the widget structure as seen by eww.
    ///
    /// This may be useful if you are facing issues with how eww is interpreting your configuration,
    /// and to provide additional context to the eww developers if you are filing a bug.
    #[structopt(name = "debug")]
    ShowDebug,
}

fn parse_var_update_arg(s: &str) -> Result<(VarName, PrimitiveValue)> {
    let (name, value) = s
        .split_once('=')
        .with_context(|| format!("arguments must be in the shape `variable_name=\"new_value\"`, but got: {}", s))?;
    Ok((name.into(), PrimitiveValue::from_string(value.to_owned())))
}

impl ActionWithServer {
    pub fn into_eww_command(self) -> (app::EwwCommand, Option<crossbeam_channel::Receiver<String>>) {
        let command = match self {
            ActionWithServer::Update { mappings } => {
                app::EwwCommand::UpdateVars(mappings.into_iter().map(|x| x.into()).collect())
            }
            ActionWithServer::OpenWindow {
                window_name,
                pos,
                size,
                anchor,
            } => app::EwwCommand::OpenWindow {
                window_name,
                pos,
                size,
                anchor,
            },
            ActionWithServer::CloseWindow { window_name } => app::EwwCommand::CloseWindow { window_name },
            ActionWithServer::KillServer => app::EwwCommand::KillServer,
            ActionWithServer::ShowState => {
                let (send, recv) = crossbeam_channel::unbounded();
                return (app::EwwCommand::PrintState(send), Some(recv));
            }
            ActionWithServer::ShowDebug => {
                let (send, recv) = crossbeam_channel::unbounded();
                return (app::EwwCommand::PrintDebug(send), Some(recv));
            }
        };
        (command, None)
    }

    /// returns true if this command requires a server to already be running
    pub fn needs_server_running(&self) -> bool {
        match self {
            ActionWithServer::OpenWindow { .. } => false,
            _ => true,
        }
    }
}
