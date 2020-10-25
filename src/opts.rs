use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use crate::{
    app,
    config::WindowName,
    value::{Coords, PrimitiveValue, VarName},
};

#[derive(StructOpt, Debug, Serialize, Deserialize, PartialEq)]
pub struct Opt {
    #[structopt(subcommand)]
    pub action: Action,

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
    #[structopt(name = "logs", help = "Print and watch the eww logs")]
    Logs,
}

#[derive(StructOpt, Debug, Serialize, Deserialize, PartialEq)]
pub enum ActionWithServer {
    #[structopt(name = "update", help = "update the value of a variable, in a running eww instance")]
    Update { fieldname: VarName, value: PrimitiveValue },

    #[structopt(name = "open", help = "open a window")]
    OpenWindow {
        window_name: WindowName,

        #[structopt(short, long, help = "The position of the window, where it should open.")]
        pos: Option<Coords>,

        #[structopt(short, long, help = "The size of the window to open")]
        size: Option<Coords>,
    },

    #[structopt(name = "close", help = "close the window with the given name")]
    CloseWindow { window_name: WindowName },

    #[structopt(name = "kill", help("kill the eww daemon"))]
    KillServer,

    #[structopt(name = "state", help = "Print the current eww-state")]
    ShowState,

    #[structopt(name = "debug", help = "Print out the widget structure as seen by eww")]
    ShowDebug,
}

impl ActionWithServer {
    pub fn into_eww_command(self) -> (app::EwwCommand, Option<crossbeam_channel::Receiver<String>>) {
        let command = match self {
            ActionWithServer::Update { fieldname, value } => app::EwwCommand::UpdateVar(fieldname, value),
            ActionWithServer::OpenWindow { window_name, pos, size } => app::EwwCommand::OpenWindow { window_name, pos, size },
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
