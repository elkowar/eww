use crate::{app, config, eww_state::*, ipc_server, script_var_handler, try_logging_errors, util};
use anyhow::*;
use futures_util::StreamExt;
use std::{
    collections::HashMap,
    os::unix::io::AsRawFd,
    path::{Path, PathBuf},
};
use tokio::sync::mpsc::*;

pub fn initialize_server(config_dir_override: Option<std::path::PathBuf>) -> Result<()> {
    do_detach()?;

    simple_signal::set_handler(&[simple_signal::Signal::Int, simple_signal::Signal::Term], move |_| {
        println!("Shutting down eww daemon...");
        if let Err(e) = crate::application_lifecycle::send_exit() {
            eprintln!("Failed to send application shutdown event to workers: {:?}", e);
            std::process::exit(1);
        }
    });
    let (ui_send, mut ui_recv) = tokio::sync::mpsc::unbounded_channel();

    let config_file_path = config_dir_override.unwrap_or(crate::CONFIG_DIR.join("eww.xml"));

    let config_dir = config_file_path
        .parent()
        .context("config file did not have a parent?!")?
        .to_owned();
    std::env::set_current_dir(&config_dir).with_context(|| { format!("Failed to change working directory to {}", config_dir.display()) } )?;
    let scss_file_path = config_dir.join("eww.scss");

    log::info!("reading configuration from {:?}", &config_file_path);
    let eww_config = config::EwwConfig::read_from_file(&config_file_path)?;

    gtk::init()?;

    log::info!("Initializing script var handler");
    let script_var_handler = script_var_handler::init(ui_send.clone());

    let mut app = app::App {
        eww_state: EwwState::from_default_vars(eww_config.generate_initial_state()?),
        eww_config,
        open_windows: HashMap::new(),
        css_provider: gtk::CssProvider::new(),
        script_var_handler,
        app_evt_send: ui_send.clone(),
        config_file_path,
        scss_file_path,
    };

    if let Some(screen) = gdk::Screen::get_default() {
        gtk::StyleContext::add_provider_for_screen(&screen, &app.css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    if let Ok(eww_css) = util::parse_scss_from_file(&app.scss_file_path) {
        app.load_css(&eww_css)?;
    }

    // initialize all the handlers and tasks running asyncronously
    init_async_part(app.config_file_path.clone(), app.scss_file_path.clone(), ui_send);

    glib::MainContext::default().spawn_local(async move {
        while let Some(event) = ui_recv.recv().await {
            app.handle_command(event);
        }
    });

    gtk::main();
    log::info!("main application thread finished");

    Ok(())
}

fn init_async_part(config_file_path: PathBuf, scss_file_path: PathBuf, ui_send: UnboundedSender<app::DaemonCommand>) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to initialize tokio runtime");
        rt.block_on(async {
            let filewatch_join_handle = {
                let ui_send = ui_send.clone();
                tokio::spawn(async move { run_filewatch(config_file_path, scss_file_path, ui_send).await })
            };

            let ipc_server_join_handle = {
                let ui_send = ui_send.clone();
                tokio::spawn(async move { ipc_server::run_server(ui_send).await })
            };

            let forward_exit_to_app_handle = {
                let ui_send = ui_send.clone();
                tokio::spawn(async move {
                    // Wait for application exit event
                    let _ = crate::application_lifecycle::recv_exit().await;
                    log::info!("Forward task received exit event");
                    // Then forward that to the application
                    let _ = ui_send.send(app::DaemonCommand::KillServer);
                })
            };

            let result = tokio::try_join!(filewatch_join_handle, ipc_server_join_handle, forward_exit_to_app_handle);

            if let Err(e) = result {
                eprintln!("Eww exiting with error: {:?}", e);
            }
        })
    });
}

/// Watch configuration files for changes, sending reload events to the eww app when the files change.
async fn run_filewatch<P: AsRef<Path>>(
    config_file_path: P,
    scss_file_path: P,
    evt_send: UnboundedSender<app::DaemonCommand>,
) -> Result<()> {
    let mut inotify = inotify::Inotify::init().context("Failed to initialize inotify")?;
    let config_file_descriptor = inotify
        .add_watch(config_file_path.as_ref(), inotify::WatchMask::MODIFY)
        .context("Failed to add inotify watch for config file")?;
    let scss_file_descriptor = inotify
        .add_watch(scss_file_path.as_ref(), inotify::WatchMask::MODIFY)
        .context("Failed to add inotify watch for scss file")?;

    let mut buffer = [0; 1024];
    let mut event_stream = inotify.event_stream(&mut buffer)?;

    crate::loop_select_exiting! {
        Some(Ok(event)) = event_stream.next() => {
            try_logging_errors!("handling change of config file" => {
                if event.wd == config_file_descriptor {
                        log::info!("Reloading eww configuration");
                        let new_eww_config = config::EwwConfig::read_from_file(config_file_path.as_ref())?;
                        evt_send.send(app::DaemonCommand::UpdateConfig(new_eww_config))?;
                } else if event.wd == scss_file_descriptor {
                        log::info!("reloading eww css file");
                        let eww_css = crate::util::parse_scss_from_file(scss_file_path.as_ref())?;
                        evt_send.send(app::DaemonCommand::UpdateCss(eww_css))?;
                } else {
                    eprintln!("Got inotify event for unknown thing: {:?}", event);
                }
            });
        }
        else => break,
    }
    Ok(())
}

/// detach the process from the terminal, also redirecting stdout and stderr to LOG_FILE
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
        .open(&*crate::LOG_FILE)
        .expect(&format!(
            "Error opening log file ({}), for writing",
            &*crate::LOG_FILE.to_string_lossy()
        ));
    let fd = file.as_raw_fd();

    if nix::unistd::isatty(1)? {
        nix::unistd::dup2(fd, std::io::stdout().as_raw_fd())?;
    }
    if nix::unistd::isatty(2)? {
        nix::unistd::dup2(fd, std::io::stderr().as_raw_fd())?;
    }

    Ok(())
}
