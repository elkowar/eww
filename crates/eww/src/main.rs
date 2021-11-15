#![feature(trace_macros)]
#![feature(drain_filter)]
#![feature(box_syntax)]
#![feature(box_patterns)]
#![feature(slice_concat_trait)]
#![feature(result_cloned)]
#![feature(try_blocks)]
#![feature(nll)]
#![allow(rustdoc::private_intra_doc_links)]

extern crate gio;
extern crate gtk;
#[cfg(feature = "wayland")]
extern crate gtk_layer_shell as gtk_layer_shell;

use anyhow::*;
use daemon_response::{DaemonResponse, DaemonResponseReceiver};
use opts::ActionWithServer;
use std::{
    os::unix::net,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::server::ForkResult;

pub mod app;
pub mod application_lifecycle;
pub mod client;
pub mod config;
mod daemon_response;
pub mod display_backend;
pub mod error;
mod error_handling_ctx;
pub mod geometry;
pub mod ipc_server;
pub mod opts;
pub mod script_var_handler;
pub mod server;
pub mod state;
pub mod util;
pub mod widgets;

fn main() {
    let eww_binary_name = std::env::args().next().unwrap();
    let opts: opts::Opt = opts::Opt::from_env();

    let log_level_filter = if opts.log_debug { log::LevelFilter::Debug } else { log::LevelFilter::Info };
    if std::env::var("RUST_LOG").is_ok() {
        pretty_env_logger::init_timed();
    } else {
        pretty_env_logger::formatted_timed_builder().filter(Some("eww"), log_level_filter).init();
    }

    let result: Result<()> = try {
        let paths = opts
            .config_path
            .map(EwwPaths::from_config_dir)
            .unwrap_or_else(EwwPaths::default)
            .context("Failed to initialize eww paths")?;

        let should_restart = match &opts.action {
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
                let fork_result = server::initialize_server(paths.clone(), None, !opts.no_daemonize)?;
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
                        let fork_result = server::initialize_server(paths.clone(), Some(command), true)?;
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
    };

    if let Err(e) = result {
        error_handling_ctx::print_error(e);
        std::process::exit(1);
    }
}

fn listen_for_daemon_response(mut recv: DaemonResponseReceiver) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().expect("Failed to initialize tokio runtime");
    rt.block_on(async {
        if let Ok(Some(response)) = tokio::time::timeout(Duration::from_millis(100), recv.recv()).await {
            println!("{}", response);
        }
    })
}

/// attempt to send a command to the daemon and send it the given action repeatedly.
fn handle_server_command(paths: &EwwPaths, action: &ActionWithServer, connect_attempts: usize) -> Result<Option<DaemonResponse>> {
    log::debug!("Trying to find server process at socket {}", paths.get_ipc_socket_file().display());
    let mut stream = attempt_connect(&paths.get_ipc_socket_file(), connect_attempts).context("Failed to connect to daemon")?;
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

#[derive(Debug, Clone)]
pub struct EwwPaths {
    log_file: PathBuf,
    ipc_socket_file: PathBuf,
    config_dir: PathBuf,
}

impl EwwPaths {
    pub fn from_config_dir<P: AsRef<Path>>(config_dir: P) -> Result<Self> {
        let config_dir = config_dir.as_ref();
        if config_dir.is_file() {
            bail!("Please provide the path to the config directory, not a file within it")
        }

        if !config_dir.exists() {
            bail!("Configuration directory {} does not exist", config_dir.display());
        }

        let config_dir = config_dir.canonicalize()?;
        let daemon_id = base64::encode(format!("{}", config_dir.display()));

        Ok(EwwPaths {
            config_dir,
            log_file: std::env::var("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap()).join(".cache"))
                .join(format!("eww_{}.log", daemon_id)),
            ipc_socket_file: std::env::var("XDG_RUNTIME_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
                .join(format!("eww-server_{}", daemon_id)),
        })
    }

    pub fn default() -> Result<Self> {
        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap()).join(".config"))
            .join("eww");

        Self::from_config_dir(config_dir)
    }

    pub fn get_log_file(&self) -> &Path {
        self.log_file.as_path()
    }

    pub fn get_ipc_socket_file(&self) -> &Path {
        self.ipc_socket_file.as_path()
    }

    pub fn get_config_dir(&self) -> &Path {
        self.config_dir.as_path()
    }

    pub fn get_yuck_path(&self) -> PathBuf {
        self.config_dir.join("eww.yuck")
    }

    pub fn get_eww_scss_path(&self) -> PathBuf {
        self.config_dir.join("eww.scss")
    }
}

impl std::fmt::Display for EwwPaths {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "config-dir: {}, ipc-socket: {}, log-file: {}",
            self.config_dir.display(),
            self.ipc_socket_file.display(),
            self.log_file.display()
        )
    }
}
