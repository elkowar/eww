use crate::{
    app::{self, App, DaemonCommand},
    config, daemon_response,
    display_backend::DisplayBackend,
    error_handling_ctx, ipc_server, script_var_handler,
    state::scope_graph::ScopeGraph,
    EwwPaths,
};
use anyhow::{Context, Result};

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    io::Write,
    marker::PhantomData,
    os::unix::io::AsRawFd,
    path::Path,
    rc::Rc,
    sync::{atomic::Ordering, Arc},
};
use tokio::sync::mpsc::*;

pub fn initialize_server<B: DisplayBackend>(
    paths: EwwPaths,
    action: Option<DaemonCommand>,
    should_daemonize: bool,
) -> Result<ForkResult> {
    let (ui_send, mut ui_recv) = tokio::sync::mpsc::unbounded_channel();

    std::env::set_current_dir(paths.get_config_dir())
        .with_context(|| format!("Failed to change working directory to {}", paths.get_config_dir().display()))?;

    log::info!("Loading paths: {}", &paths);

    let read_config = config::read_from_eww_paths(&paths);

    let eww_config = match read_config {
        Ok(config) => config,
        Err(err) => {
            error_handling_ctx::print_error(err);
            config::EwwConfig::default()
        }
    };

    cleanup_log_dir(paths.get_log_dir())?;

    if should_daemonize {
        let fork_result = do_detach(paths.get_log_file())?;

        if fork_result == ForkResult::Parent {
            return Ok(ForkResult::Parent);
        }
    }

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

    if B::IS_WAYLAND {
        std::env::set_var("GDK_BACKEND", "wayland")
    }
    gtk::init()?;

    log::debug!("Initializing script var handler");
    let script_var_handler = script_var_handler::init(ui_send.clone());

    let (scope_graph_evt_send, mut scope_graph_evt_recv) = tokio::sync::mpsc::unbounded_channel();

    let mut app: App<B> = app::App {
        scope_graph: Rc::new(RefCell::new(ScopeGraph::from_global_vars(
            eww_config.generate_initial_state()?,
            scope_graph_evt_send,
        ))),
        eww_config,
        open_windows: HashMap::new(),
        failed_windows: HashSet::new(),
        instance_id_to_args: HashMap::new(),
        css_provider: gtk::CssProvider::new(),
        script_var_handler,
        app_evt_send: ui_send.clone(),
        window_close_timer_abort_senders: HashMap::new(),
        paths,
        phantom: PhantomData,
    };

    if let Some(screen) = gtk::gdk::Screen::default() {
        gtk::StyleContext::add_provider_for_screen(&screen, &app.css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    if let Ok((file_id, css)) = config::scss::parse_scss_from_config(app.paths.get_config_dir()) {
        if let Err(e) = app.load_css(file_id, &css) {
            error_handling_ctx::print_error(e);
        }
    }

    connect_monitor_added(ui_send.clone());

    // initialize all the handlers and tasks running asyncronously
    let tokio_handle = init_async_part(app.paths.clone(), ui_send);

    gtk::glib::MainContext::default().spawn_local(async move {
        // if an action was given to the daemon initially, execute it first.
        if let Some(action) = action {
            app.handle_command(action).await;
        }

        loop {
            tokio::select! {
                Some(scope_graph_evt) = scope_graph_evt_recv.recv() => {
                    app.scope_graph.borrow_mut().handle_scope_graph_event(scope_graph_evt);
                },
                Some(ui_event) = ui_recv.recv() => {
                    app.handle_command(ui_event).await;
                }
                else => break,
            }
        }
    });

    // allow the GTK main thread to do tokio things
    let _g = tokio_handle.enter();

    gtk::main();
    log::info!("main application thread finished");

    Ok(ForkResult::Child)
}

fn connect_monitor_added(ui_send: UnboundedSender<DaemonCommand>) {
    let display = gtk::gdk::Display::default().expect("could not get default display");
    display.connect_monitor_added({
        move |_display: &gtk::gdk::Display, _monitor: &gtk::gdk::Monitor| {
            log::info!("New monitor connected, reloading configuration");
            let _ = reload_config_and_css(&ui_send);
        }
    });
}

fn reload_config_and_css(ui_send: &UnboundedSender<DaemonCommand>) -> Result<()> {
    let (daemon_resp_sender, mut daemon_resp_response) = daemon_response::create_pair();
    ui_send.send(DaemonCommand::ReloadConfigAndCss(daemon_resp_sender))?;
    tokio::spawn(async move {
        match daemon_resp_response.recv().await {
            Some(daemon_response::DaemonResponse::Success(_)) => log::info!("Reloaded config successfully"),
            Some(daemon_response::DaemonResponse::Failure(e)) => eprintln!("{}", e),
            None => log::error!("No response to reload configuration-reload request"),
        }
    });
    Ok(())
}

fn init_async_part(paths: EwwPaths, ui_send: UnboundedSender<app::DaemonCommand>) -> tokio::runtime::Handle {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .thread_name("main-async-runtime")
        .enable_all()
        .build()
        .expect("Failed to initialize tokio runtime");
    let handle = rt.handle().clone();

    std::thread::Builder::new()
        .name("outer-main-async-runtime".to_string())
        .spawn(move || {
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
                        log::debug!("Forward task received exit event");
                        // Then forward that to the application
                        let _ = ui_send.send(app::DaemonCommand::KillServer);
                    })
                };

                let result = tokio::try_join!(filewatch_join_handle, ipc_server_join_handle, forward_exit_to_app_handle);

                if let Err(e) = result {
                    log::error!("Eww exiting with error: {:?}", e);
                }
            })
        })
        .expect("Failed to start outer-main-async-runtime thread");

    handle
}

/// Watch configuration files for changes, sending reload events to the eww app when the files change.
async fn run_filewatch<P: AsRef<Path>>(config_dir: P, evt_send: UnboundedSender<app::DaemonCommand>) -> Result<()> {
    use notify::{RecommendedWatcher, RecursiveMode, Watcher};

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
        Ok(notify::Event { kind: notify::EventKind::Modify(_), paths, .. }) => {
            let relevant_files_changed = paths.iter().any(|path| {
                let ext = path.extension().unwrap_or_default();
                ext == "yuck" || ext == "scss" || ext == "css"
            });
            if relevant_files_changed {
                if let Err(err) = tx.send(()) {
                    log::warn!("Error forwarding file update event: {:?}", err);
                }
            }
        }
        Ok(_) => {}
        Err(e) => log::error!("Encountered Error While Watching Files: {}", e),
    })?;
    watcher.watch(config_dir.as_ref(), RecursiveMode::Recursive)?;

    // make sure to not trigger reloads too much by only accepting one reload every 500ms.
    let debounce_done = Arc::new(std::sync::atomic::AtomicBool::new(true));

    crate::loop_select_exiting! {
        Some(()) = rx.recv() => {
            let debounce_done = debounce_done.clone();
            if debounce_done.swap(false, Ordering::SeqCst) {
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    debounce_done.store(true, Ordering::SeqCst);
                });

                // without this sleep, reading the config file sometimes gives an empty file.
                // This is probably a result of editors not locking the file correctly,
                // and eww being too fast, thus reading the file while it's empty.
                // There should be some cleaner solution for this, but this will do for now.
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                reload_config_and_css(&evt_send)?;
            }
        },
        else => break
    };
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ForkResult {
    Parent,
    Child,
}

/// detach the process from the terminal, also redirecting stdout and stderr to LOG_FILE
fn do_detach(log_file_path: impl AsRef<Path>) -> Result<ForkResult> {
    // detach from terminal
    match unsafe { nix::unistd::fork()? } {
        nix::unistd::ForkResult::Child => {
            nix::unistd::setsid()?;
            match unsafe { nix::unistd::fork()? } {
                nix::unistd::ForkResult::Parent { .. } => std::process::exit(0),
                nix::unistd::ForkResult::Child => {}
            }
        }
        nix::unistd::ForkResult::Parent { .. } => {
            return Ok(ForkResult::Parent);
        }
    }

    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
        .unwrap_or_else(|_| panic!("Error opening log file ({}), for writing", log_file_path.as_ref().to_string_lossy()));
    let fd = file.as_raw_fd();

    if nix::unistd::isatty(1)? {
        nix::unistd::dup2(fd, std::io::stdout().as_raw_fd())?;
    }
    if nix::unistd::isatty(2)? {
        nix::unistd::dup2(fd, std::io::stderr().as_raw_fd())?;
    }

    Ok(ForkResult::Child)
}

/// Ensure the log directory never grows larger than 100MB by deleting files older than 7 days,
/// and truncating all other logfiles to 100MB.
fn cleanup_log_dir(log_dir: impl AsRef<Path>) -> Result<()> {
    // Find all files named "eww_*.log" in the log directory
    let log_files = std::fs::read_dir(&log_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                if file_name.to_string_lossy().starts_with("eww_") && file_name.to_string_lossy().ends_with(".log") {
                    Some(path)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    for log_file in log_files {
        // if the file is older than a week, delete it
        if let Ok(metadata) = log_file.metadata() {
            if metadata.modified()?.elapsed()?.as_secs() > 60 * 60 * 24 * 7 {
                log::info!("Deleting old log file: {}", log_file.display());
                std::fs::remove_file(&log_file)?;
            } else {
                // If the file is larger than 200MB, delete the start of it until it's 100MB or less.
                let mut file = std::fs::OpenOptions::new().append(true).open(&log_file)?;
                let file_size = file.metadata()?.len();
                if file_size > 200_000_000 {
                    let mut file_content = std::fs::read(&log_file)?;
                    let bytes_to_remove = file_content.len().saturating_sub(100_000_000);
                    file_content.drain(0..bytes_to_remove);
                    file.set_len(0)?;
                    file.write_all(&file_content)?;
                }
            }
        }
    }
    Ok(())
}
