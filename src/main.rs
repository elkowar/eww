#![feature(trace_macros)]
#![feature(slice_concat_trait)]
#![feature(result_cloned)]
#![feature(iterator_fold_self)]
#![feature(try_blocks)]
#![feature(str_split_once)]

extern crate gio;
extern crate gtk;

use anyhow::*;

use std::{os::unix::net, path::PathBuf};

pub mod app;
pub mod application_lifecycle;
pub mod client;
pub mod config;
pub mod eww_state;
pub mod ipc_server;
pub mod opts;
pub mod script_var_handler;
pub mod server;
pub mod util;
pub mod value;
pub mod widgets;

lazy_static::lazy_static! {
    pub static ref IPC_SOCKET_PATH: std::path::PathBuf = std::env::var("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
        .join("eww-server");

    pub static ref CONFIG_DIR: std::path::PathBuf = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap()).join(".config"))
        .join("eww");

    pub static ref LOG_FILE: std::path::PathBuf = std::env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap()).join(".cache"))
        .join("eww.log");
}

fn main() {
    let opts: opts::Opt = opts::Opt::from_env();

    let log_level_filter = if opts.log_debug {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Off
    };

    pretty_env_logger::formatted_builder()
        .filter(Some("eww"), log_level_filter)
        .init();

    let result: Result<_> = try {
        match opts.action {
            opts::Action::ClientOnly(action) => {
                client::handle_client_only_action(action)?;
            }
            opts::Action::WithServer(action) => {
                log::info!("Trying to find server process");
                match net::UnixStream::connect(&*IPC_SOCKET_PATH) {
                    Ok(stream) => {
                        log::info!("Connected to Eww server.");
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

            opts::Action::Daemon { config } => {
                // make sure that there isn't already a Eww daemon running.
                if check_server_running(&*IPC_SOCKET_PATH) {
                    eprintln!("Eww server already running.");
                    std::process::exit(1);
                } else {
                    log::info!("Initializing Eww server.");
                    let _ = std::fs::remove_file(&*crate::IPC_SOCKET_PATH);

                    println!("Run `eww logs` to see any errors, warnings or information while editing your configuration.");
                    server::initialize_server(config)?;
                }
            }
        }
    };

    if let Err(e) = result {
        eprintln!("{:?}", e);
        std::process::exit(1);
    }
}

/// Check if a eww server is currently running by trying to send a ping message to it.
fn check_server_running(socket_path: &std::path::PathBuf) -> bool {
    let response = net::UnixStream::connect(socket_path)
        .ok()
        .and_then(|stream| client::do_server_call(stream, opts::ActionWithServer::Ping).ok());
    response.is_some()
}
