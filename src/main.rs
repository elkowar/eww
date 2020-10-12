#![feature(trace_macros)]
#![feature(result_cloned)]
#![feature(iterator_fold_self)]
#![feature(try_blocks)]
#![feature(str_split_once)]

extern crate gio;
extern crate gtk;

use anyhow::*;
use eww_state::*;
use gdk::*;
use gtk::prelude::*;
use hotwatch;
use ipc_channel::ipc;
use log;
use pretty_env_logger;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use structopt::StructOpt;
use value::{PrimitiveValue, VarName};

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

fn main() {
    pretty_env_logger::init();
    if let Err(e) = try_main() {
        eprintln!("{:?}", e);
    }
}

#[derive(StructOpt, Debug, Serialize, Deserialize)]
pub struct Opt {
    #[structopt(short = "-c", parse(from_os_str))]
    config_file: Option<PathBuf>,

    #[structopt(subcommand)]
    action: OptAction,

    #[structopt(short = "-d", long = "--detach")]
    should_detach: bool,
}
#[derive(StructOpt, Debug, Serialize, Deserialize)]
pub enum OptAction {
    #[structopt(name = "update")]
    Update { fieldname: VarName, value: PrimitiveValue },

    #[structopt(name = "open")]
    OpenWindow {
        window_name: config::WindowName,

        #[structopt(short, long)]
        pos: Option<util::Coords>,

        #[structopt(short, long)]
        size: Option<util::Coords>,
    },

    #[structopt(name = "close")]
    CloseWindow { window_name: config::WindowName },

    #[structopt(name = "kill")]
    KillServer,
}

fn try_main() -> Result<()> {
    let opts: Opt = StructOpt::from_args();
    log::info!("Trying to find server process");
    if let Ok(sender) = find_server_process() {
        log::info!("Forwarding options to server");
        sender.send(opts)?;
    } else {
        log::info!("No instance found... Initializing server.");

        if opts.should_detach {
            do_detach();
        }

        initialize_server(opts)?;
    }
    Ok(())
}

fn find_server_process() -> Result<ipc::IpcSender<Opt>> {
    let instance_path = std::fs::read_to_string("/tmp/eww-instance-path")?;
    Ok(ipc::IpcSender::connect(instance_path)?)
}

fn get_config_file_path() -> PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .map(|v| PathBuf::from(v))
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap()).join(".config"))
        .join("eww")
        .join("eww.xml")
}

fn initialize_server(opts: Opt) -> Result<()> {
    let config_file_path = opts.config_file.clone().unwrap_or_else(get_config_file_path);
    let config_dir = config_file_path
        .clone()
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
    script_var_handler.setup_command_poll_tasks(&eww_config)?;

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
    app.handle_user_command(&opts)?;

    run_server_thread(evt_send.clone());
    let _hotwatch = run_filewatch_thread(&config_file_path, &scss_file_path, evt_send.clone())?;

    evt_recv.attach(None, move |msg| {
        app.handle_event(msg);
        glib::Continue(true)
    });

    gtk::main();

    Ok(())
}

fn run_server_thread(evt_send: glib::Sender<app::EwwEvent>) {
    std::thread::spawn(move || {
        log::info!("Starting up eww server");
        let result: Result<_> = try {
            loop {
                let (ipc_server, instance_path): (ipc::IpcOneShotServer<Opt>, _) = ipc::IpcOneShotServer::new()?;
                std::fs::write("/tmp/eww-instance-path", instance_path)?;
                let (_, initial) = ipc_server.accept()?;
                log::info!("received command from IPC: {:?}", &initial);
                evt_send.send(app::EwwEvent::UserCommand(initial))?;
            }
        };
        if let Err(err) = result {
            eprintln!("error in server thread: {}", err);
            std::process::exit(1);
        }
    });
}

fn run_filewatch_thread<P: AsRef<Path>>(
    config_file_path: P,
    scss_file_path: P,
    evt_send: glib::Sender<app::EwwEvent>,
) -> Result<hotwatch::Hotwatch> {
    log::info!("Initializing config file watcher");
    let mut hotwatch = hotwatch::Hotwatch::new()?;

    let config_file_change_send = evt_send.clone();
    hotwatch.watch_file_changes(config_file_path, move |path| {
        try_logging_errors!("handling change of config file" => {
            log::info!("Reloading eww configuration");
            let new_eww_config = config::EwwConfig::read_from_file(path)?;
            config_file_change_send.send(app::EwwEvent::ReloadConfig(new_eww_config))?;
        });
    })?;

    let result = hotwatch.watch_file_changes(scss_file_path, move |path| {
        try_logging_errors!("handling change of scss file" =>  {
            log::info!("reloading eww css file");
            let eww_css = util::parse_scss_from_file(path)?;
            evt_send.send(app::EwwEvent::ReloadCss(eww_css))?;
        })
    });
    if let Err(e) = result {
        eprintln!("WARN: error while loading CSS file for hot-reloading: \n{}", e)
    };
    Ok(hotwatch)
}

/// detach the process from the terminal, also closing stdout and redirecting
/// stderr into /dev/null
fn do_detach() {
    // detach from terminal
    let pid = unsafe { libc::fork() };
    if dbg!(pid) < 0 {
        panic!("Phailed to Phork: {}", std::io::Error::last_os_error());
    }
    if pid != 0 {
        std::process::exit(0);
    }

    // close stdout to not spam output
    if unsafe { libc::isatty(1) } != 0 {
        unsafe {
            libc::close(1);
        }
    }
    // close stderr to not spam output
    if unsafe { libc::isatty(2) } != 0 {
        unsafe {
            let fd = libc::open(std::ffi::CString::new("/dev/null").unwrap().as_ptr(), libc::O_RDWR);
            if fd < 0 {
                panic!("Phailed to open /dev/null?!: {}", std::io::Error::last_os_error());
            } else {
                if libc::dup2(fd, libc::STDERR_FILENO) < 0 {
                    panic!(
                        "Phailed to dup stderr phd to /dev/null: {:?}",
                        std::io::Error::last_os_error()
                    );
                }
                libc::close(fd);
            }
        }
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
