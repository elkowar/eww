#![feature(trace_macros)]
#![feature(slice_concat_trait)]
#![feature(result_cloned)]
#![feature(iterator_fold_self)]
#![feature(try_blocks)]
#![feature(str_split_once)]

extern crate gio;
extern crate gtk;

use crate::value::{PrimitiveValue, VarName};
use anyhow::*;
use config::window_definition::WindowName;
use eww_state::*;
use log;
use pretty_env_logger;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::{Read, Write},
    os::unix::{io::AsRawFd, net},
    path::{Path, PathBuf},
    process::Stdio,
};
use structopt::StructOpt;
use value::Coords;

pub mod app;
pub mod config;
pub mod eww_state;
pub mod script_var_handler;
pub mod util;
pub mod value;
pub mod widgets;

#[macro_export]
macro_rules! build {
    ($var_name:ident = $value:expr ; $($code:tt)*) => {{
        let mut $var_name = $value;
        $($code)*
        $var_name
    }};
}

macro_rules! try_logging_errors {
    ($context:literal => $code:block) => {{
        let result: Result<_> = try { $code };
        if let Err(err) = result {
            eprintln!("Error while {}: {:?}", $context, err);
        }
    }};
}

lazy_static::lazy_static! {
    static ref IPC_SOCKET_PATH: std::path::PathBuf = std::env::var("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
        .join("eww-server");

    static ref CONFIG_DIR: std::path::PathBuf = std::env::var("XDG_CONFIG_HOME")
        .map(|v| PathBuf::from(v))
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap()).join(".config"))
        .join("eww");

    static ref LOG_FILE: std::path::PathBuf = std::env::var("XDG_CACHE_HOME")
        .map(|v| PathBuf::from(v))
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap()).join(".cache"))
        .join("eww.log");
}

fn main() {
    pretty_env_logger::init();
    if let Err(e) = try_main() {
        eprintln!("{:?}", e);
    }
}

#[derive(StructOpt, Debug, Serialize, Deserialize, PartialEq)]
pub struct Opt {
    #[structopt(subcommand)]
    action: OptAction,

    #[structopt(short = "-d", long = "--detach")]
    should_detach: bool,
}

#[derive(StructOpt, Debug, Serialize, Deserialize, PartialEq)]
pub enum OptAction {
    #[structopt(flatten)]
    ClientOnly(OptActionClientOnly),
    #[structopt(flatten)]
    WithServer(OptActionWithServer),
}

#[derive(StructOpt, Debug, Serialize, Deserialize, PartialEq)]
pub enum OptActionClientOnly {
    #[structopt(name = "logs", help = "Print and watch the eww logs")]
    Logs,
}

#[derive(StructOpt, Debug, Serialize, Deserialize, PartialEq)]
pub enum OptActionWithServer {
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

impl OptActionWithServer {
    fn into_eww_command(self) -> (app::EwwCommand, Option<crossbeam_channel::Receiver<String>>) {
        let command = match self {
            OptActionWithServer::Update { fieldname, value } => app::EwwCommand::UpdateVar(fieldname, value),
            OptActionWithServer::OpenWindow { window_name, pos, size } => app::EwwCommand::OpenWindow { window_name, pos, size },
            OptActionWithServer::CloseWindow { window_name } => app::EwwCommand::CloseWindow { window_name },
            OptActionWithServer::KillServer => app::EwwCommand::KillServer,
            OptActionWithServer::ShowState => {
                let (send, recv) = crossbeam_channel::unbounded();
                return (app::EwwCommand::PrintState(send), Some(recv));
            }
            OptActionWithServer::ShowDebug => {
                let (send, recv) = crossbeam_channel::unbounded();
                return (app::EwwCommand::PrintDebug(send), Some(recv));
            }
        };
        (command, None)
    }

    /// returns true if this command requires a server to already be running
    fn needs_server_running(&self) -> bool {
        match self {
            OptActionWithServer::OpenWindow { .. } => false,
            _ => true,
        }
    }
}

fn try_main() -> Result<()> {
    let opts: Opt = StructOpt::from_args();

    match opts.action {
        OptAction::ClientOnly(action) => {
            handle_client_only_action(action)?;
        }
        OptAction::WithServer(action) => {
            log::info!("Trying to find server process");
            if let Ok(mut stream) = net::UnixStream::connect(&*IPC_SOCKET_PATH) {
                log::info!("Forwarding options to server");
                stream.write_all(&bincode::serialize(&action)?)?;

                let mut buf = String::new();
                stream.set_read_timeout(Some(std::time::Duration::from_millis(100)))?;
                stream.read_to_string(&mut buf)?;
                println!("{}", buf);
            } else {
                log::info!("No instance found... Initializing server.");

                let _ = std::fs::remove_file(&*IPC_SOCKET_PATH);

                if opts.should_detach {
                    do_detach()?;
                }

                initialize_server(action)?;
            }
        }
    }
    Ok(())
}

fn handle_client_only_action(action: OptActionClientOnly) -> Result<()> {
    match action {
        OptActionClientOnly::Logs => {
            std::process::Command::new("tail")
                .args(["-f", LOG_FILE.to_string_lossy().as_ref()].iter())
                .stdin(Stdio::null())
                .spawn()?
                .wait()?;
        }
    }
    Ok(())
}

fn initialize_server(action: OptActionWithServer) -> Result<()> {
    if action.needs_server_running() {
        println!("No eww server running");
        return Ok(());
    }

    let config_file_path = CONFIG_DIR.join("eww.xml");
    let config_dir = config_file_path
        .parent()
        .context("config file did not have a parent?!")?
        .to_owned()
        .to_path_buf();
    let scss_file_path = config_dir.join("eww.scss");

    log::info!("reading configuration from {:?}", &config_file_path);
    let eww_config = config::EwwConfig::read_from_file(&config_file_path)?;

    gtk::init()?;
    let (evt_send, evt_recv) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

    log::info!("Initializing script var handler");
    let mut script_var_handler = script_var_handler::ScriptVarHandler::new(evt_send.clone())?;
    script_var_handler.initialize_clean(eww_config.get_script_vars().clone())?;

    let mut app = app::App {
        eww_state: EwwState::from_default_vars(eww_config.generate_initial_state()?.clone()),
        eww_config,
        windows: HashMap::new(),
        css_provider: gtk::CssProvider::new(),
        script_var_handler,
        app_evt_send: evt_send.clone(),
    };

    if let Some(screen) = gdk::Screen::get_default() {
        gtk::StyleContext::add_provider_for_screen(&screen, &app.css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    if let Ok(eww_css) = util::parse_scss_from_file(&scss_file_path) {
        app.load_css(&eww_css)?;
    }

    // run the command that eww was started with
    log::info!("running command: {:?}", &action);
    let (command, maybe_response_recv) = action.into_eww_command();
    app.handle_command(command);
    if let Some(response_recv) = maybe_response_recv {
        if let Ok(response) = response_recv.recv_timeout(std::time::Duration::from_millis(100)) {
            println!("{}", response);
        }
    }

    run_server_thread(evt_send.clone())?;
    let _hotwatch = run_filewatch_thread(&config_file_path, &scss_file_path, evt_send.clone())?;

    evt_recv.attach(None, move |msg| {
        app.handle_command(msg);
        glib::Continue(true)
    });

    gtk::main();

    Ok(())
}

fn run_server_thread(evt_send: glib::Sender<app::EwwCommand>) -> Result<()> {
    std::thread::spawn(move || {
        let result: Result<_> = try {
            log::info!("Starting up eww server");
            let listener = net::UnixListener::bind(&*IPC_SOCKET_PATH)?;
            for stream in listener.incoming() {
                try_logging_errors!("handling message from IPC client" => {
                    let mut stream = stream?;
                    let action: OptActionWithServer = bincode::deserialize_from(&stream)?;
                    log::info!("received command from IPC: {:?}", &action);
                    let (command, maybe_response_recv) = action.into_eww_command();
                    evt_send.send(command)?;
                    if let Some(response_recv) = maybe_response_recv {
                        if let Ok(response) = response_recv.recv_timeout(std::time::Duration::from_millis(100)) {
                            let result = &stream.write_all(response.as_bytes());
                            util::print_result_err("Sending text response to ipc client", &result);
                        }
                    }
                });
            }
        };
        if let Err(err) = result {
            eprintln!("error in server thread: {}", err);
            std::process::exit(1);
        }
    });
    Ok(())
}

fn run_filewatch_thread<P: AsRef<Path>>(
    config_file_path: P,
    scss_file_path: P,
    evt_send: glib::Sender<app::EwwCommand>,
) -> Result<hotwatch::Hotwatch> {
    log::info!("Initializing config file watcher");
    let mut hotwatch = hotwatch::Hotwatch::new()?;

    let config_file_change_send = evt_send.clone();
    hotwatch.watch_file_changes(config_file_path, move |path| {
        try_logging_errors!("handling change of config file" => {
            log::info!("Reloading eww configuration");
            let new_eww_config = config::EwwConfig::read_from_file(path)?;
            config_file_change_send.send(app::EwwCommand::ReloadConfig(new_eww_config))?;
        });
    })?;

    let result = hotwatch.watch_file_changes(scss_file_path, move |path| {
        try_logging_errors!("handling change of scss file" =>  {
            log::info!("reloading eww css file");
            let eww_css = util::parse_scss_from_file(path)?;
            evt_send.send(app::EwwCommand::ReloadCss(eww_css))?;
        })
    });
    util::print_result_err("while loading CSS file for hot-reloading", &result);
    Ok(hotwatch)
}

/// detach the process from the terminal, also redirecting stdout and stderr to
/// LOG_FILE
fn do_detach() -> Result<()> {
    // detach from terminal
    match unsafe { nix::unistd::fork()? } {
        nix::unistd::ForkResult::Parent { .. } => {
            std::process::exit(0);
        }
        nix::unistd::ForkResult::Child => {}
    }

    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&*LOG_FILE)
        .expect(&format!(
            "Error opening log file ({}), for writing",
            &*LOG_FILE.to_string_lossy()
        ));
    let fd = file.as_raw_fd();

    if nix::unistd::isatty(1)? {
        nix::unistd::dup2(std::io::stdout().as_raw_fd(), fd)?;
    }
    if nix::unistd::isatty(2)? {
        nix::unistd::dup2(std::io::stderr().as_raw_fd(), fd)?;
    }

    nix::unistd::setsid().context("Failed to run setsid")?;
    Ok(())
}

#[extend::ext(pub)]
impl hotwatch::Hotwatch {
    fn watch_file_changes<P, F>(&mut self, file_path: P, callback: F) -> Result<()>
    where
        P: AsRef<Path>,
        F: 'static + Fn(PathBuf) + Send,
    {
        Ok(self.watch(file_path, move |evt| match evt {
            hotwatch::Event::Write(path) | hotwatch::Event::NoticeWrite(path) => callback(path),
            _ => {}
        })?)
    }
}
