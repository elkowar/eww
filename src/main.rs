#![feature(trace_macros)]
#![feature(try_blocks)]
extern crate gio;
extern crate gtk;

use anyhow::*;
use gdk::*;
use gio::prelude::*;
use gtk::prelude::*;
use ipc_channel::ipc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

const EXAMPLE_CONFIG: &str = r#"{
    widgets: {
        some_widget: {
            structure: {
                layout: {
                    class: "container",
                    children: [
                        { layout: {
                            orientation: "v"
                            children: [
                                "fancy button"
                                { button: { children: "reeee" } }
                            ]
                        } }
                        { layout: {
                            children: [
                                "date" // TODO FIX!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                                { label: { text: "$$date" } }
                                { button: { children: "click me you" } }
                                { slider: { value: "$$some_value", min: 0, max: 100, onchange: "notify-send 'changed' {}" } }
                                { slider: { value: "$$some_value", orientation: "vertical" } }
                                "hu"
                            ]
                        } }
                    ]
                }
            }
        },
        test: {
            structure: {
                some_widget: {
                    some_value: "$$ooph"
                }
            }
        }
    },
    default_vars: {
        ree: 12
        date: "never"
    }
    windows: {
        main_window: {
            pos.x: 200
            pos.y: 1550
            size.x: 500
            size.y: 50
            widget: {
                test: {
                    ooph: "$$ree"
                }
            }
        }
    },
}"#;

fn main() {
    if let Err(e) = try_main() {
        eprintln!("{:?}", e);
    }
}

#[derive(StructOpt, Debug, Serialize, Deserialize)]
struct Opt {
    #[structopt(subcommand)]
    action: OptAction,
}
#[derive(StructOpt, Debug, Serialize, Deserialize)]
enum OptAction {
    #[structopt(name = "update")]
    Update(OptActionUpdate),

    #[structopt(name = "open")]
    OpenWindow(OptActionOpen),
}

#[derive(StructOpt, Debug, Serialize, Deserialize)]
struct OptActionUpdate {
    fieldname: String,
    value: PrimitiveValue,
}

#[derive(StructOpt, Debug, Serialize, Deserialize)]
struct OptActionOpen {
    window_name: String,
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

fn initialize_server(opts: Opt) -> Result<()> {
    let eww_config = config::EwwConfig::from_hocon(&config::parse_hocon(EXAMPLE_CONFIG)?)?;

    let mut app = App {
        eww_state: EwwState::from_default_vars(eww_config.get_default_vars().clone()),
        eww_config,
    };

    let (send, recv) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

    gtk::init()?;

    app.handle_user_command(opts)?;

    std::thread::spawn(move || run_ipc_server(send));
    recv.attach(None, move |msg| {
        app.handle_event(msg);
        glib::Continue(true)
    });

    gtk::main();

    Ok(())
}

fn run_ipc_server(send: glib::Sender<EwwEvent>) -> Result<()> {
    loop {
        let (ipc_server, instance_path): (ipc::IpcOneShotServer<Opt>, _) = ipc::IpcOneShotServer::new()?;
        std::fs::write("/tmp/eww-instance-path", instance_path)?;
        let (receiver, initial) = ipc_server.accept()?;
        send.send(EwwEvent::UserCommand(initial))?;
    }
}

#[derive(Debug)]
struct App {
    eww_state: EwwState,
    eww_config: config::EwwConfig,
}

impl App {
    fn handle_user_command(&mut self, opts: Opt) -> Result<()> {
        match opts.action {
            OptAction::Update(update) => self.update_state(update),
            OptAction::OpenWindow(update) => self.open_window(update)?,
        }
        Ok(())
    }

    fn update_state(&mut self, update: OptActionUpdate) {
        self.eww_state.update_value(update.fieldname, update.value);
    }

    fn open_window(&mut self, open_window: OptActionOpen) -> Result<()> {
        let window_def = self.eww_config.get_windows()[&open_window.window_name].clone();

        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_title("Eww");
        window.set_wmclass("noswallow", "noswallow");
        window.set_type_hint(gdk::WindowTypeHint::Dock);
        window.set_position(gtk::WindowPosition::Center);
        window.set_keep_above(true);
        window.set_default_size(window_def.size.0, window_def.size.1);
        window.set_visual(
            window
                .get_display()
                .get_default_screen()
                .get_rgba_visual()
                .or_else(|| window.get_display().get_default_screen().get_system_visual())
                .as_ref(),
        );

        window.fullscreen();

        let empty_local_state = HashMap::new();

        window.add(&widgets::element_to_gtk_thing(
            &self.eww_config.get_widgets(),
            &mut self.eww_state,
            &empty_local_state,
            &window_def.widget,
        )?);

        window.show_all();

        let gdk_window = window.get_window().unwrap();
        gdk_window.set_override_redirect(true);
        gdk_window.move_(window_def.position.0, window_def.position.1);
        gdk_window.show();
        gdk_window.raise();

        Ok(())
    }

    fn handle_event(&mut self, event: EwwEvent) {
        let result: Result<_> = try {
            match event {
                EwwEvent::UserCommand(command) => self.handle_user_command(command)?,
            }
        };
        if let Err(err) = result {
            eprintln!("Error while handling event: {:?}", err);
        }
    }
}

#[derive(Debug)]
enum EwwEvent {
    UserCommand(Opt),
}

fn event_loop(sender: glib::Sender<EwwEvent>) {
    let mut x = 0;
    loop {
        x += 1;
        std::thread::sleep(std::time::Duration::from_millis(1000));
        let event_opt = Opt {
            action: OptAction::Update(OptActionUpdate {
                fieldname: "ree".to_string(),
                value: PrimitiveValue::Number(x as f64 * 10.0),
            }),
        };
        sender.send(EwwEvent::UserCommand(event_opt)).unwrap();
    }
}
