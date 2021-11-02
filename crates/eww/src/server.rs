use crate::{EwwPaths, app::{self, DaemonCommand}, config, daemon_response, error_handling_ctx, ipc_server, script_var_handler, state::scope_graph::ScopeGraph, util};
use anyhow::*;

use debug_cell::RefCell;
use std::{
    collections::{HashMap, HashSet},
    os::unix::io::AsRawFd,
    path::Path,
    rc::Rc,
    sync::{atomic::Ordering, Arc},
};
use tokio::sync::mpsc::*;

pub fn initialize_server(paths: EwwPaths, action: Option<DaemonCommand>, should_daemonize: bool) -> Result<ForkResult> {
    let (ui_send, mut ui_recv) = tokio::sync::mpsc::unbounded_channel();

    std::env::set_current_dir(&paths.get_config_dir())
        .with_context(|| format!("Failed to change working directory to {}", paths.get_config_dir().display()))?;

    log::info!("Loading paths: {}", &paths);

    let read_config = config::read_from_file(&paths.get_yuck_path());

    let eww_config = match read_config {
        Ok(config) => config,
        Err(err) => {
            error_handling_ctx::print_error(err);
            config::EwwConfig::default()
        }
    };

    if should_daemonize {
        let fork_result = do_detach(&paths.get_log_file())?;

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

    gtk::init()?;

    log::debug!("Initializing script var handler");
    let script_var_handler = script_var_handler::init(ui_send.clone());

    let (scope_graph_evt_send, mut scope_graph_evt_recv) = tokio::sync::mpsc::unbounded_channel();

    let mut app = app::App {
        scope_graph: Rc::new(RefCell::new(ScopeGraph::from_global_vars(eww_config.generate_initial_state()?, scope_graph_evt_send))),
        eww_config,
        open_windows: HashMap::new(),
        failed_windows: HashSet::new(),
        css_provider: gtk::CssProvider::new(),
        script_var_handler,
        app_evt_send: ui_send.clone(),
        paths,
    };

    if let Some(screen) = gdk::Screen::default() {
        gtk::StyleContext::add_provider_for_screen(&screen, &app.css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    if let Ok(eww_css) = util::parse_scss_from_file(&app.paths.get_eww_scss_path()) {
        app.load_css(&eww_css)?;
    }

    // initialize all the handlers and tasks running asyncronously
    init_async_part(app.paths.clone(), ui_send);

    glib::MainContext::default().spawn_local(async move {
        // if an action was given to the daemon initially, execute it first.
        if let Some(action) = action {
            app.handle_command(action);
        }

        loop {
            tokio::select! {
                Some(scope_graph_evt) = scope_graph_evt_recv.recv() => {
                    app.scope_graph.borrow_mut().handle_scope_graph_event(scope_graph_evt);
                },
                Some(ui_event) = ui_recv.recv() => {
                    app.handle_command(ui_event);
                }
                else => break,
            }
        }
    });

    gtk::main();
    log::info!("main application thread finished");

    Ok(ForkResult::Child)
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
    });
}

/// Watch configuration files for changes, sending reload events to the eww app when the files change.
async fn run_filewatch<P: AsRef<Path>>(config_dir: P, evt_send: UnboundedSender<app::DaemonCommand>) -> Result<()> {
    use notify::{RecommendedWatcher, RecursiveMode, Watcher};

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut watcher: RecommendedWatcher = Watcher::new(move |res: notify::Result<notify::Event>| match res {
        Ok(notify::Event { kind: notify::EventKind::Modify(_), paths, .. }) => {
            let relevant_files_changed = paths.iter().any(|path| {
                let ext = path.extension().unwrap_or_default();
                ext == "yuck" || ext == "scss"
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

                let (daemon_resp_sender, mut daemon_resp_response) = daemon_response::create_pair();
                // without this sleep, reading the config file sometimes gives an empty file.
                // This is probably a result of editors not locking the file correctly,
                // and eww being too fast, thus reading the file while it's empty.
                // There should be some cleaner solution for this, but this will do for now.
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                evt_send.send(app::DaemonCommand::ReloadConfigAndCss(daemon_resp_sender))?;
                tokio::spawn(async move {
                    match daemon_resp_response.recv().await {
                        Some(daemon_response::DaemonResponse::Success(_)) => log::info!("Reloaded config successfully"),
                        Some(daemon_response::DaemonResponse::Failure(e)) => eprintln!("{}", e),
                        None => log::error!("No response to reload configuration-reload request"),
                    }
                });
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
