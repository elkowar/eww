#![allow(rustdoc::private_intra_doc_links)]

extern crate gtk;
#[cfg(feature = "wayland")]
extern crate gtk_layer_shell as gtk_layer_shell;

use anyhow::{Context, Result};
use clap::CommandFactory as _;
use daemon_response::{DaemonResponse, DaemonResponseReceiver};
use display_backend::DisplayBackend;
use opts::ActionWithServer;
use paths::EwwPaths;
use std::{os::unix::net, path::Path, time::Duration};

use crate::server::ForkResult;

mod app;
mod application_lifecycle;
mod client;
mod config;
mod daemon_response;
mod display_backend;
mod error_handling_ctx;
mod file_database;
mod geometry;
mod ipc_server;
mod opts;
mod paths;
mod script_var_handler;
mod server;
mod state;
mod util;
mod widgets;
mod window_arguments;
mod window_initiator;

fn main() {
    let eww_binary_name = std::env::args().next().unwrap();
    let opts: opts::Opt = opts::Opt::from_env();

    let log_level_filter = if opts.log_debug { log::LevelFilter::Debug } else { log::LevelFilter::Info };
    if std::env::var("RUST_LOG").is_ok() {
        pretty_env_logger::init_timed();
    } else {
        pretty_env_logger::formatted_timed_builder()
            .filter(Some("eww"), log_level_filter)
            .filter(Some("notifier_host"), log_level_filter)
            .init();
    }

    if let opts::Action::ShellCompletions { shell } = opts.action {
        clap_complete::generate(shell, &mut opts::RawOpt::command(), "eww", &mut std::io::stdout());
        return;
    }

    let detected_wayland = detect_wayland();
    #[allow(unused)]
    let use_wayland = opts.force_wayland || detected_wayland;
    #[cfg(all(feature = "wayland", feature = "x11"))]
    let result = if use_wayland {
        log::debug!("Running on wayland. force_wayland={}, detected_wayland={}", opts.force_wayland, detected_wayland);
        run::<display_backend::WaylandBackend>(opts, eww_binary_name)
    } else {
        log::debug!("Running on X11. force_wayland={}, detected_wayland={}", opts.force_wayland, detected_wayland);
        run::<display_backend::X11Backend>(opts, eww_binary_name)
    };

    #[cfg(all(not(feature = "wayland"), feature = "x11"))]
    let result = {
        if use_wayland {
            log::warn!("Eww compiled without wayland support. Falling back to X11, eventhough wayland was requested.");
        }
        run::<display_backend::X11Backend>(opts, eww_binary_name)
    };

    #[cfg(all(feature = "wayland", not(feature = "x11")))]
    let result = run::<display_backend::WaylandBackend>(opts, eww_binary_name);

    #[cfg(not(any(feature = "wayland", feature = "x11")))]
    let result = run::<display_backend::NoBackend>(opts, eww_binary_name);

    if let Err(err) = result {
        error_handling_ctx::print_error(err);
        std::process::exit(1);
    }
}

fn detect_wayland() -> bool {
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let wayland_display = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
    session_type.contains("wayland") || (!wayland_display.is_empty() && !session_type.contains("x11"))
}

fn run<B: DisplayBackend>(opts: opts::Opt, eww_binary_name: String) -> Result<()> {
    let paths = opts
        .config_path
        .map(EwwPaths::from_config_dir)
        .unwrap_or_else(EwwPaths::default)
        .context("Failed to initialize eww paths")?;

    let should_restart = match &opts.action {
        opts::Action::ShellCompletions { .. } => unreachable!(),
        opts::Action::Daemon => opts.restart,
        opts::Action::WithServer(action) => opts.restart && action.can_start_daemon(),
        opts::Action::ClientOnly(_) => false,
    };
    if should_restart {
        let response = handle_server_command(&paths, &ActionWithServer::KillServer, 1);
        if let Ok(Some(response)) = response {
            handle_daemon_response(response);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    let would_show_logs = match opts.action {
        opts::Action::ShellCompletions { .. } => unreachable!(),
        opts::Action::ClientOnly(action) => {
            client::handle_client_only_action(&paths, action)?;
            false
        }

        // make sure that there isn't already a Eww daemon running.
        opts::Action::Daemon if check_server_running(paths.get_ipc_socket_file()) => {
            eprintln!("Eww server already running.");
            true
        }
        opts::Action::Daemon => {
            log::info!("Initializing Eww server. ({})", paths.get_ipc_socket_file().display());
            let _ = std::fs::remove_file(paths.get_ipc_socket_file());

            if !opts.show_logs {
                println!("Run `{} logs` to see any errors while editing your configuration.", eww_binary_name);
            }
            let fork_result = server::initialize_server::<B>(paths.clone(), None, !opts.no_daemonize)?;
            opts.no_daemonize || fork_result == ForkResult::Parent
        }

        opts::Action::WithServer(ActionWithServer::KillServer) => {
            if let Some(response) = handle_server_command(&paths, &ActionWithServer::KillServer, 1)? {
                handle_daemon_response(response);
            }
            false
        }

        // a running daemon is necessary for this command
        opts::Action::WithServer(action) => {
            // attempt to just send the command to a running daemon
            match handle_server_command(&paths, &action, 5) {
                Ok(Some(response)) => {
                    handle_daemon_response(response);
                    true
                }
                Ok(None) => true,

                Err(err) if action.can_start_daemon() && !opts.no_daemonize => {
                    // connecting to the daemon failed. Thus, start the daemon here!
                    log::warn!("Failed to connect to daemon: {}", err);
                    log::info!("Initializing eww server. ({})", paths.get_ipc_socket_file().display());
                    let _ = std::fs::remove_file(paths.get_ipc_socket_file());
                    if !opts.show_logs {
                        println!("Run `{} logs` to see any errors while editing your configuration.", eww_binary_name);
                    }

                    let (command, response_recv) = action.into_daemon_command();
                    // start the daemon and give it the command
                    let fork_result = server::initialize_server::<B>(paths.clone(), Some(command), true)?;
                    let is_parent = fork_result == ForkResult::Parent;
                    if let (Some(recv), true) = (response_recv, is_parent) {
                        listen_for_daemon_response(recv);
                    }
                    is_parent
                }
                Err(err) => Err(err)?,
            }
        }
    };

    if would_show_logs && opts.show_logs {
        client::handle_client_only_action(&paths, opts::ActionClientOnly::Logs)?;
    }
    Ok(())
}

fn listen_for_daemon_response(mut recv: DaemonResponseReceiver) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .thread_name("listen-for-daemon-response")
        .enable_all()
        .build()
        .expect("Failed to initialize tokio runtime");
    rt.block_on(async {
        if let Ok(Some(response)) = tokio::time::timeout(Duration::from_millis(100), recv.recv()).await {
            println!("{}", response);
        }
    })
}

/// attempt to send a command to the daemon and send it the given action repeatedly.
fn handle_server_command(paths: &EwwPaths, action: &ActionWithServer, connect_attempts: usize) -> Result<Option<DaemonResponse>> {
    log::debug!("Trying to find server process at socket {}", paths.get_ipc_socket_file().display());
    let mut stream = attempt_connect(paths.get_ipc_socket_file(), connect_attempts).context("Failed to connect to daemon")?;
    log::debug!("Connected to Eww server ({}).", &paths.get_ipc_socket_file().display());
    client::do_server_call(&mut stream, action).context("Error while forwarding command to server")
}

fn handle_daemon_response(res: DaemonResponse) {
    match res {
        DaemonResponse::Success(x) => println!("{}", x),
        DaemonResponse::Failure(x) => {
            eprintln!("{}", x);
            std::process::exit(1);
        }
    }
}

fn attempt_connect(socket_path: impl AsRef<Path>, attempts: usize) -> Option<net::UnixStream> {
    for _ in 0..attempts {
        if let Ok(mut con) = net::UnixStream::connect(&socket_path) {
            if client::do_server_call(&mut con, &opts::ActionWithServer::Ping).is_ok() {
                return net::UnixStream::connect(&socket_path).ok();
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    None
}

/// Check if a eww server is currently running by trying to send a ping message to it.
fn check_server_running(socket_path: impl AsRef<Path>) -> bool {
    let response = net::UnixStream::connect(socket_path)
        .ok()
        .and_then(|mut stream| client::do_server_call(&mut stream, &opts::ActionWithServer::Ping).ok());
    response.is_some()
}
