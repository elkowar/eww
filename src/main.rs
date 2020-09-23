#![feature(trace_macros)]
#![feature(try_blocks)]
extern crate gio;
extern crate gtk;

use anyhow::*;
use gdk::*;
use gio::prelude::*;
use grass;
use gtk::prelude::*;
use ipc_channel::ipc;
use notify::{self, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path;
use structopt::StructOpt;

pub mod config;
pub mod eww_state;
pub mod value;
pub mod widgets;

use eww_state::*;
use value::PrimitiveValue;

#[macro_export]
macro_rules! build {
    ($var_name:ident = $value:expr ; $code:block) => {{
        let mut $var_name = $value;
        $code;
        $var_name
    }};
}

fn main() {
    if let Err(e) = try_main() {
        eprintln!("{:?}", e);
    }
}

#[derive(StructOpt, Debug, Serialize, Deserialize)]
struct Opt {
    #[structopt(short = "-c", parse(from_os_str))]
    config_file: Option<path::PathBuf>,

    #[structopt(subcommand)]
    action: OptAction,
}
#[derive(StructOpt, Debug, Serialize, Deserialize)]
enum OptAction {
    #[structopt(name = "update")]
    Update { fieldname: String, value: PrimitiveValue },

    #[structopt(name = "open")]
    OpenWindow { window_name: String },

    #[structopt(name = "close")]
    CloseWindow { window_name: String },
}

#[derive(Debug)]
enum EwwEvent {
    UserCommand(Opt),
    ReloadConfig(config::EwwConfig),
    ReloadCss(String),
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

fn get_config_file_path() -> path::PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .map(|v| path::PathBuf::from(v))
        .unwrap_or_else(|_| path::PathBuf::from(std::env::var("HOME").unwrap()).join(".config"))
        .join("eww")
        .join("eww.conf")
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

    let (watcher_tx, watcher_rx) = std::sync::mpsc::channel();

    let mut file_watcher = notify::watcher(watcher_tx, std::time::Duration::from_millis(100))?;
    file_watcher.watch(config_file_path.clone(), notify::RecursiveMode::NonRecursive)?;
    if let Err(e) = file_watcher.watch(scss_file_path.clone(), notify::RecursiveMode::NonRecursive) {
        eprintln!("WARN: error while loading CSS file for hot-reloading: \n{}", e)
    }

    let config_content = std::fs::read_to_string(config_file_path.clone())?;
    let scss_content = std::fs::read_to_string(scss_file_path.clone()).unwrap_or_default();

    let eww_config = config::EwwConfig::from_hocon(&config::parse_hocon(&config_content)?)?;
    let eww_css =
        grass::from_string(scss_content, &grass::Options::default()).map_err(|err| anyhow!("SCSS parsing error: {:?}", err))?;

    let mut app = App {
        eww_state: EwwState::from_default_vars(eww_config.get_default_vars().clone()),
        eww_config,
        eww_css: eww_css.clone(),
        windows: HashMap::new(),
    };
    gtk::init()?;

    let css_provider = gtk::CssProvider::new();
    css_provider.load_from_data(eww_css.as_bytes())?;
    gdk::Screen::get_default().map(|screen| {
        gtk::StyleContext::add_provider_for_screen(&screen, &css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    });

    app.handle_user_command(opts)?;

    let (send, recv) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    std::thread::spawn({
        let send = send.clone();
        {
            move || {
                let result: Result<_> = try {
                    loop {
                        let (ipc_server, instance_path): (ipc::IpcOneShotServer<Opt>, _) = ipc::IpcOneShotServer::new()?;
                        std::fs::write("/tmp/eww-instance-path", instance_path)?;
                        let (_, initial) = ipc_server.accept()?;
                        send.send(EwwEvent::UserCommand(initial))?;
                    }
                };
                if let Err(err) = result {
                    eprintln!("error in server thread: {}", err);
                    std::process::exit(1);
                }
            }
        }
    });
    std::thread::spawn(move || {
        while let Ok(event) = watcher_rx.recv() {
            let result: Result<_> = try {
                dbg!(&event);
                match event {
                    notify::DebouncedEvent::Write(updated_path) | notify::DebouncedEvent::NoticeWrite(updated_path)
                        if updated_path == config_file_path =>
                    {
                        let new_eww_config = config::EwwConfig::from_hocon(&config::parse_hocon(&config_content)?)?;
                        send.send(EwwEvent::ReloadConfig(new_eww_config))?;
                    }
                    notify::DebouncedEvent::Write(updated_path) if updated_path == scss_file_path => {
                        let scss_content = std::fs::read_to_string(scss_file_path.clone()).unwrap_or_default();
                        let eww_css = grass::from_string(scss_content, &grass::Options::default())
                            .map_err(|err| anyhow!("SCSS parsing error: {:?}", err))?;
                        send.send(EwwEvent::ReloadCss(eww_css))?;
                    }
                    _ => {}
                }
            };
            if let Err(err) = result {
                eprintln!("error in server thread: {}", err);
                std::process::exit(1);
            }
        }
    });

    recv.attach(None, move |msg| {
        app.handle_event(msg);
        glib::Continue(true)
    });

    gtk::main();

    Ok(())
}

#[derive(Debug)]
struct App {
    eww_state: EwwState,
    eww_config: config::EwwConfig,
    eww_css: String,
    windows: HashMap<String, gtk::Window>,
}

impl App {
    fn handle_user_command(&mut self, opts: Opt) -> Result<()> {
        match opts.action {
            OptAction::Update { fieldname, value } => self.update_state(fieldname, value),
            OptAction::OpenWindow { window_name } => self.open_window(&window_name)?,
            OptAction::CloseWindow { window_name } => self.close_window(&window_name)?,
        }
        Ok(())
    }

    fn update_state(&mut self, fieldname: String, value: PrimitiveValue) {
        self.eww_state.update_value(fieldname, value);
    }

    fn close_window(&mut self, window_name: &str) -> Result<()> {
        let window = self
            .windows
            .get(window_name)
            .context(format!("No window with name '{}' is running.", window_name))?;
        window.close();
        Ok(())
    }

    fn open_window(&mut self, window_name: &str) -> Result<()> {
        let window_def = self
            .eww_config
            .get_windows()
            .get(window_name)
            .context(format!("No window named '{}' defined", window_name))?
            .clone();

        let window = gtk::Window::new(gtk::WindowType::Popup);
        window.set_title("Eww");
        window.set_wmclass("noswallow", "noswallow");
        window.set_type_hint(gdk::WindowTypeHint::Dock);
        window.set_position(gtk::WindowPosition::Center);
        window.set_default_size(window_def.size.0, window_def.size.1);
        window.set_decorated(false);

        let empty_local_state = HashMap::new();
        let root_widget = &widgets::element_to_gtk_thing(
            &self.eww_config.get_widgets(),
            &mut self.eww_state,
            &empty_local_state,
            &window_def.widget,
        )?;
        root_widget.get_style_context().add_class(window_name);
        window.add(root_widget);

        window.show_all();

        let gdk_window = window.get_window().unwrap();
        gdk_window.set_override_redirect(true);
        gdk_window.move_(window_def.position.0, window_def.position.1);
        gdk_window.show();
        gdk_window.raise();
        window.set_keep_above(true);

        self.windows.insert(window_name.to_string(), window);

        Ok(())
    }

    fn reload_all_windows(&mut self, config: config::EwwConfig) -> Result<()> {
        self.eww_config = config;
        let windows = self.windows.clone();
        for (window_name, window) in windows {
            dbg!(&window_name);
            window.close();
            window.hide();
            self.open_window(&window_name)?;
        }
        Ok(())
    }

    fn reload_css(&mut self, css: String) -> Result<()> {
        for window in self.windows.values() {}
        Ok(())
    }

    fn handle_event(&mut self, event: EwwEvent) {
        let result: Result<_> = try {
            match event {
                EwwEvent::UserCommand(command) => self.handle_user_command(command)?,
                EwwEvent::ReloadConfig(config) => self.reload_all_windows(config)?,
                EwwEvent::ReloadCss(css) => self.reload_css(css)?,
            }
        };
        if let Err(err) = result {
            eprintln!("Error while handling event: {:?}", err);
        }
    }
}
