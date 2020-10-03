#![feature(trace_macros)]
#![feature(try_blocks)]
extern crate gio;
extern crate gtk;

use anyhow::*;
use eww_state::*;
use gdk::*;
use gtk::prelude::*;
use hotwatch;
use ipc_channel::ipc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use value::PrimitiveValue;

pub mod app;
pub mod config;
pub mod eww_state;
pub mod util;
pub mod value;
pub mod widgets;

#[macro_export]
macro_rules! build {
    ($var_name:ident = $value:expr ; $code:block) => {{
        let mut $var_name = $value;
        $code;
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
}
#[derive(StructOpt, Debug, Serialize, Deserialize)]
pub enum OptAction {
    #[structopt(name = "update")]
    Update { fieldname: String, value: PrimitiveValue },

    #[structopt(name = "open")]
    OpenWindow { window_name: String },

    #[structopt(name = "close")]
    CloseWindow { window_name: String },
}

fn try_main() -> Result<()> {
    let opts: Opt = StructOpt::from_args();
    if let Ok(sender) = find_server_process() {
        sender.send(opts)?;
    } else {
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

    let eww_config = config::EwwConfig::read_from_file(&config_file_path)?;

    gtk::init()?;

    let mut app = app::App {
        eww_state: EwwState::from_default_vars(eww_config.get_default_vars().clone()),
        eww_config,
        windows: HashMap::new(),
        css_provider: gtk::CssProvider::new(),
    };

    if let Some(screen) = gdk::Screen::get_default() {
        gtk::StyleContext::add_provider_for_screen(&screen, &app.css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    if let Ok(eww_css) = util::parse_scss_from_file(&scss_file_path) {
        app.load_css(&eww_css)?;
    }

    // run the command that eww was started with
    app.handle_user_command(opts)?;

    let (evt_send, evt_recv) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    run_server_thread(evt_send.clone());
    run_filewatch_thread(&config_file_path, &scss_file_path, evt_send.clone())?;

    evt_recv.attach(None, move |msg| {
        app.handle_event(msg);
        glib::Continue(true)
    });

    gtk::main();

    Ok(())
}

fn run_server_thread(evt_send: glib::Sender<app::EwwEvent>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let result: Result<_> = try {
            loop {
                let (ipc_server, instance_path): (ipc::IpcOneShotServer<Opt>, _) = ipc::IpcOneShotServer::new()?;
                std::fs::write("/tmp/eww-instance-path", instance_path)?;
                let (_, initial) = ipc_server.accept()?;
                evt_send.send(app::EwwEvent::UserCommand(initial))?;
            }
        };
        if let Err(err) = result {
            eprintln!("error in server thread: {}", err);
            std::process::exit(1);
        }
    })
}

fn run_filewatch_thread<P: AsRef<Path>>(
    config_file_path: P,
    scss_file_path: P,
    evt_send: glib::Sender<app::EwwEvent>,
) -> Result<()> {
    let mut hotwatch = hotwatch::Hotwatch::new()?;
    hotwatch.watch_file_changes(
        config_file_path,
        glib::clone!(@strong evt_send => move |path| {
            try_logging_errors!("handling change of config file" => {
                let new_eww_config = config::EwwConfig::read_from_file(path)?;
                evt_send.send(app::EwwEvent::ReloadConfig(new_eww_config))?;
            });
        }),
    )?;

    let result = hotwatch.watch_file_changes(scss_file_path, move |path| {
        try_logging_errors!("handling change of scss file" =>  {
            let eww_css = util::parse_scss_from_file(path)?;
            evt_send.send(app::EwwEvent::ReloadCss(eww_css))?;
        })
    });
    if let Err(e) = result {
        eprintln!("WARN: error while loading CSS file for hot-reloading: \n{}", e)
    };
    Ok(())
}

#[extend::ext(pub)]
impl hotwatch::Hotwatch {
    fn watch_file_changes<P, F>(&mut self, path: P, callback: F) -> Result<()>
    where
        P: AsRef<Path>,
        F: 'static + Fn(PathBuf) + Send,
    {
        let result = self.watch(path, move |evt| match evt {
            hotwatch::Event::Write(path) | hotwatch::Event::NoticeWrite(path) => callback(path),
            _ => {}
        });
        Ok(result?)
    }
}
