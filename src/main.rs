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
use eww_state::*;
use log;
use pretty_env_logger;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::{Read, Write},
    os::unix::{io::AsRawFd, net},
    path::{Path, PathBuf},
};
use structopt::StructOpt;

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
    #[structopt(name = "update", help = "update the value of a variable, in a running eww instance")]
    Update { fieldname: VarName, value: PrimitiveValue },

    #[structopt(name = "open", help = "open a window")]
    OpenWindow {
        window_name: config::WindowName,

        #[structopt(short, long, help = "The position of the window, where it should open.")]
        pos: Option<util::Coords>,

        #[structopt(short, long, help = "The size of the window to open")]
        size: Option<util::Coords>,
    },

    #[structopt(name = "close", help = "close the window with the given name")]
    CloseWindow { window_name: config::WindowName },

    #[structopt(name = "kill", help = "kill the eww daemon")]
    KillServer,

    #[structopt(name = "state", help = "Print the current eww-state")]
    ShowState,

    #[structopt(name = "debug", help = "Print out the widget structure as seen by eww")]
    ShowDebug,
}

impl OptAction {
    fn into_eww_command(self) -> (app::EwwCommand, Option<crossbeam_channel::Receiver<String>>) {
        match self {
            OptAction::Update { fieldname, value } => (app::EwwCommand::UpdateVar(fieldname, value), None),
            OptAction::OpenWindow { window_name, pos, size } => (app::EwwCommand::OpenWindow { window_name, pos, size }, None),
            OptAction::CloseWindow { window_name } => (app::EwwCommand::CloseWindow { window_name }, None),
            OptAction::KillServer => (app::EwwCommand::KillServer, None),
            OptAction::ShowState => {
                let (send, recv) = crossbeam_channel::unbounded();
                (app::EwwCommand::PrintState(send), Some(recv))
            }
            OptAction::ShowDebug => {
                let (send, recv) = crossbeam_channel::unbounded();
                (app::EwwCommand::PrintDebug(send), Some(recv))
            }
        }
    }
}

fn try_main() -> Result<()> {
    let opts: Opt = StructOpt::from_args();
    log::info!("Trying to find server process");
    if let Ok(mut stream) = net::UnixStream::connect(&*IPC_SOCKET_PATH) {
        log::info!("Forwarding options to server");
        stream.write_all(&bincode::serialize(&opts)?)?;

        let mut buf = String::new();
        stream.read_to_string(&mut buf)?;
        println!("{}", buf);
    } else {
        log::info!("No instance found... Initializing server.");

        let _ = std::fs::remove_file(&*IPC_SOCKET_PATH);

        if opts.should_detach {
            do_detach();
        }

        initialize_server(opts)?;
    }
    Ok(())
}

fn initialize_server(opts: Opt) -> Result<()> {
    if opts.action == OptAction::KillServer {
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
    let (command, maybe_response_recv) = opts.action.into_eww_command();
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
                    let opts: Opt = bincode::deserialize_from(&stream)?;
                    log::info!("received command from IPC: {:?}", &opts);
                    let (command, maybe_response_recv) = opts.action.into_eww_command();
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
fn do_detach() {
    // detach from terminal
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        panic!("Phailed to Phork: {}", std::io::Error::last_os_error());
    }
    if pid != 0 {
        std::process::exit(0);
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

    unsafe {
        if libc::isatty(1) != 0 {
            if libc::dup2(fd, libc::STDOUT_FILENO) < 0 {
                panic!("Phailed to dup stdout to log file: {:?}", std::io::Error::last_os_error());
            }
        }
        if libc::isatty(2) != 0 {
            if libc::dup2(fd, libc::STDERR_FILENO) < 0 {
                panic!("Phailed to dup stderr to log file: {:?}", std::io::Error::last_os_error());
            }
        }
        libc::close(fd);
    }
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
