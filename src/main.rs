#![feature(trace_macros)]
#![feature(box_syntax)]
#![feature(box_patterns)]
#![feature(slice_concat_trait)]
#![feature(result_cloned)]
#![feature(try_blocks)]
#![feature(nll)]

extern crate gio;
extern crate gtk;
#[cfg(feature = "wayland")]
extern crate gtk_layer_shell as gtk_layer_shell;

use anyhow::*;
use std::{
    os::unix::net,
    path::{Path, PathBuf},
};

pub mod app;
pub mod application_lifecycle;
pub mod client;
pub mod config;
pub mod display_backend;
pub mod eww_state;
pub mod geometry;
pub mod ipc_server;
pub mod opts;
pub mod script_var_handler;
pub mod server;
pub mod util;
pub mod value;
pub mod widgets;

fn main() {
    let opts: opts::Opt = opts::Opt::from_env();

    let log_level_filter = if opts.log_debug { log::LevelFilter::Debug } else { log::LevelFilter::Info };
    if std::env::var("RUST_LOG").is_ok() {
        println!("hey");
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

        match opts.action {
            opts::Action::ClientOnly(action) => {
                client::handle_client_only_action(&paths, action)?;
            }
            opts::Action::WithServer(action) => {
                log::info!("Trying to find server process at socket {}", paths.get_ipc_socket_file().display());
                match net::UnixStream::connect(&paths.get_ipc_socket_file()) {
                    Ok(stream) => {
                        log::info!("Connected to Eww server ({}).", &paths.get_ipc_socket_file().display());
                        let response =
                            client::do_server_call(stream, action).context("Error while forwarding command to server")?;
                        if let Some(response) = response {
                            println!("{}", response);
                            if response.is_failure() {
                                std::process::exit(1);
                            }
                        }
                    }
                    Err(_) => {
                        eprintln!("Failed to connect to the eww daemon.");
                        eprintln!("Make sure to start the eww daemon process by running `eww daemon` first.");
                        std::process::exit(1);
                    }
                }
            }

            opts::Action::Daemon => {
                // make sure that there isn't already a Eww daemon running.
                if check_server_running(paths.get_ipc_socket_file()) {
                    eprintln!("Eww server already running.");
                    std::process::exit(1);
                } else {
                    log::info!("Initializing Eww server. ({})", paths.get_ipc_socket_file().display());
                    let _ = std::fs::remove_file(paths.get_ipc_socket_file());

                    println!("Run `eww logs` to see any errors, warnings or information while editing your configuration.");
                    server::initialize_server(paths)?;
                }
            }
        }
    };

    if let Err(e) = result {
        log::error!("{:?}", e);
        std::process::exit(1);
    }
}

/// Check if a eww server is currently running by trying to send a ping message to it.
fn check_server_running(socket_path: impl AsRef<Path>) -> bool {
    let response = net::UnixStream::connect(socket_path)
        .ok()
        .and_then(|stream| client::do_server_call(stream, opts::ActionWithServer::Ping).ok());
    response.is_some()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EwwPaths {
    log_file: PathBuf,
    ipc_socket_file: PathBuf,
    config_dir: PathBuf,
}

impl EwwPaths {
    pub fn from_config_dir<P: AsRef<Path>>(config_dir: P) -> Result<Self> {
        let config_dir = config_dir.as_ref();
        let config_dir = if config_dir.is_file() {
            config_dir.parent().context("Given config file did not have a parent directory")?
        } else {
            config_dir
        };

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

    pub fn get_eww_xml_path(&self) -> PathBuf {
        self.config_dir.join("eww.xml")
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
