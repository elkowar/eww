#![feature(trace_macros)]
#![feature(slice_concat_trait)]
#![feature(result_cloned)]
#![feature(iterator_fold_self)]
#![feature(try_blocks)]
#![feature(str_split_once)]


extern crate gio;
extern crate gtk;

use anyhow::*;

use log;
use pretty_env_logger;
use std::{os::unix::net, path::PathBuf};
use structopt::StructOpt;

pub mod app;
pub mod client;
pub mod config;
pub mod eww_state;
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
        .map(|v| PathBuf::from(v))
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap()).join(".config"))
        .join("eww");

    pub static ref LOG_FILE: std::path::PathBuf = std::env::var("XDG_CACHE_HOME")
        .map(|v| PathBuf::from(v))
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap()).join(".cache"))
        .join("eww.log");
}

fn main() {
    pretty_env_logger::init();

    let result: Result<_> = try {
        let opts: opts::Opt = StructOpt::from_args();

        match opts.action {
            opts::Action::ClientOnly(action) => {
                client::handle_client_only_action(action)?;
            }
            opts::Action::WithServer(action) => {
                log::info!("Trying to find server process");
                if let Ok(stream) = net::UnixStream::connect(&*IPC_SOCKET_PATH) {
                    client::forward_command_to_server(stream, action)?;
                } else {
                    if action.needs_server_running() {
                        println!("No eww server running");
                    } else {
                        log::info!("No server running, initializing server...");
                        server::initialize_server(opts.should_detach, action)?;
                    }
                }
            }
        }
    };

    if let Err(e) = result {
        eprintln!("{:?}", e);
    }
}
