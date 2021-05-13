use crate::{app, config, eww_state::*, ipc_server, script_var_handler, util, EwwPaths};
use anyhow::*;
use std::{collections::HashMap, os::unix::io::AsRawFd, path::Path};
use tokio::sync::mpsc::*;

pub fn initialize_server(paths: EwwPaths) -> Result<()> {
    do_detach(&paths.get_log_file())?;

    println!(
        r#"
┏━━━━━━━━━━━━━━━━━━━━━━━┓
┃Initializing eww daemon┃
┗━━━━━━━━━━━━━━━━━━━━━━━┛
    "#
    );

    simple_signal::set_handler(&[simple_signal::Signal::Int, simple_signal::Signal::Term], move |_| {
        log::info!("Shutting down eww daemon...");
        if let Err(e) = crate::application_lifecycle::send_exit() {
            log::error!("Failed to send application shutdown event to workers: {:?}", e);
            std::process::exit(1);
        }
    });
    let (ui_send, mut ui_recv) = tokio::sync::mpsc::unbounded_channel();

    std::env::set_current_dir(&paths.get_config_dir())
        .with_context(|| format!("Failed to change working directory to {}", paths.get_config_dir().display()))?;

    log::info!("Loading paths: {}", &paths);
    let eww_config = config::EwwConfig::read_from_file(&paths.get_eww_xml_path())?;

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
        paths,
    };

    if let Some(screen) = gdk::Screen::get_default() {
        gtk::StyleContext::add_provider_for_screen(&screen, &app.css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    if let Ok(eww_css) = util::parse_scss_from_file(&app.paths.get_eww_scss_path()) {
        app.load_css(&eww_css)?;
    }

    // initialize all the handlers and tasks running asyncronously
    init_async_part(app.paths.clone(), ui_send);

    glib::MainContext::default().spawn_local(async move {
        while let Some(event) = ui_recv.recv().await {
            app.handle_command(event);
        }
    });

    gtk::main();
    log::info!("main application thread finished");

    Ok(())
}

fn init_async_part(paths: EwwPaths, ui_send: UnboundedSender<app::DaemonCommand>) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("Failed to initialize tokio runtime");
        rt.block_on(async {
            let filewatch_join_handle = {
                let ui_send = ui_send.clone();
                let paths = paths.clone();
                tokio::spawn(async move { run_filewatch(paths.config_dir, ui_send).await })
            };

            let ipc_server_join_handle = {
                let ui_send = ui_send.clone();
                tokio::spawn(async move { ipc_server::run_server(ui_send, paths.get_ipc_socket_file()).await })
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
                log::error!("Eww exiting with error: {:?}", e);
            }
        })
    });
}

/// Watch configuration files for changes, sending reload events to the eww app when the files change.
async fn run_filewatch<P: AsRef<Path>>(config_dir: P, evt_send: UnboundedSender<app::DaemonCommand>) -> Result<()> {
    use notify::Watcher;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut watcher: notify::RecommendedWatcher =
        notify::Watcher::new_immediate(move |res: notify::Result<notify::Event>| match res {
            Ok(event) => {
                if let Err(err) = tx.send(event.paths) {
                    log::warn!("Error forwarding file update event: {:?}", err);
                }
            }
            Err(e) => log::error!("Encountered Error While Watching Files: {}", e),
        })?;
    watcher.watch(&config_dir, notify::RecursiveMode::Recursive)?;

    crate::loop_select_exiting! {
        Some(paths) = rx.recv() => {
            for path in paths {
                let extension = path.extension().unwrap_or_default();
                if extension != "xml" && extension != "scss" {
                    continue;
                }

                let (daemon_resp_sender, mut daemon_resp_response) = tokio::sync::mpsc::unbounded_channel();
                evt_send.send(app::DaemonCommand::ReloadConfigAndCss(daemon_resp_sender))?;
                tokio::spawn(async move {
                    match daemon_resp_response.recv().await {
                        Some(app::DaemonResponse::Success(_)) => log::info!("Reloaded config successfully"),
                        Some(app::DaemonResponse::Failure(e)) => log::error!("Failed to reload config: {}", e),
                        None => log::error!("No response to reload configuration-reload request"),
                    }
                });
            }
        },
        else => break
    };
    return Ok(());
}

/// detach the process from the terminal, also redirecting stdout and stderr to LOG_FILE
fn do_detach(log_file_path: impl AsRef<Path>) -> Result<()> {
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
        .open(&log_file_path)
        .expect(&format!("Error opening log file ({}), for writing", log_file_path.as_ref().to_string_lossy()));
    let fd = file.as_raw_fd();

    if nix::unistd::isatty(1)? {
        nix::unistd::dup2(fd, std::io::stdout().as_raw_fd())?;
    }
    if nix::unistd::isatty(2)? {
        nix::unistd::dup2(fd, std::io::stderr().as_raw_fd())?;
    }

    Ok(())
}
