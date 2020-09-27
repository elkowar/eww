#![feature(trace_macros)]
#![feature(try_blocks)]
extern crate gio;
extern crate gtk;

use anyhow::*;
use gdk::*;
use grass;
use gtk::prelude::*;
use ipc_channel::ipc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path;
use structopt::StructOpt;

pub mod config;
pub mod eww_state;
pub mod value;
pub mod widgets;

use eww_state::*;
use hotwatch;
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

    let (watcher_tx, watcher_rx) = crossbeam_channel::unbounded();

    let mut hotwatch = hotwatch::Hotwatch::new()?;
    hotwatch.watch(
        config_file_path.clone(),
        glib::clone!(@strong watcher_tx => move |evt| watcher_tx.send(evt).unwrap()),
    )?;

    if let Err(e) = hotwatch.watch(
        scss_file_path.clone(),
        glib::clone!(@strong watcher_tx => move |evt| watcher_tx.send(evt).unwrap()),
    ) {
        eprintln!("WARN: error while loading CSS file for hot-reloading: \n{}", e)
    }

    let config_content = std::fs::read_to_string(config_file_path.clone())?;
    let scss_content = std::fs::read_to_string(scss_file_path.clone()).unwrap_or_default();

    let eww_config = config::EwwConfig::from_hocon(&config::parse_hocon(&config_content)?)?;
    let eww_css =
        grass::from_string(scss_content, &grass::Options::default()).map_err(|err| anyhow!("SCSS parsing error: {:?}", err))?;

    gtk::init()?;

    let mut app = App {
        eww_state: EwwState::from_default_vars(eww_config.get_default_vars().clone()),
        eww_config,
        eww_css: eww_css.clone(),
        windows: HashMap::new(),
        css_provider: gtk::CssProvider::new(),
    };

    gdk::Screen::get_default().map(|screen| {
        gtk::StyleContext::add_provider_for_screen(&screen, &app.css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    });

    app.load_css(&eww_css)?;
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
                match event {
                    hotwatch::Event::Write(path) | hotwatch::Event::NoticeWrite(path) if path == config_file_path => {
                        let config_content = std::fs::read_to_string(path).unwrap_or_default();
                        let new_eww_config = config::EwwConfig::from_hocon(&config::parse_hocon(&config_content)?)?;
                        send.send(EwwEvent::ReloadConfig(new_eww_config))?;
                    }
                    hotwatch::Event::Write(path) if path == scss_file_path => {
                        let scss_content = std::fs::read_to_string(scss_file_path.clone()).unwrap_or_default();
                        let eww_css = grass::from_string(scss_content, &grass::Options::default())
                            .map_err(|err| anyhow!("SCSS parsing error: {:?}", err))?;
                        send.send(EwwEvent::ReloadCss(eww_css))?;
                    }
                    _ => {}
                }
            };
            if let Err(err) = result {
                eprintln!("error in file watcher thread: {}", err);
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
    css_provider: gtk::CssProvider,
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
        window.set_size_request(window_def.size.0, window_def.size.1);
        window.set_decorated(false);
        window.set_resizable(false);

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

        let gdk_window = window.get_window().context("couldn't get gdk window from gtk window")?;
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
        // TODO this needs to handle removing the callbacks to the old gtk windows, as otherwise this might by horribly fucked.
        let windows = self.windows.clone();
        for (window_name, window) in windows {
            window.close();
            self.open_window(&window_name)?;
        }
        Ok(())
    }

    fn load_css(&mut self, css: &str) -> Result<()> {
        self.css_provider.load_from_data(css.as_bytes())?;
        Ok(())
    }

    fn handle_event(&mut self, event: EwwEvent) {
        let result: Result<_> = try {
            match event {
                EwwEvent::UserCommand(command) => self.handle_user_command(command)?,
                EwwEvent::ReloadConfig(config) => self.reload_all_windows(config)?,
                EwwEvent::ReloadCss(css) => self.load_css(&css)?,
            }
        };
        if let Err(err) = result {
            eprintln!("Error while handling event: {:?}", err);
        }
    }
}
