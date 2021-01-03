use crate::{app, config, eww_state::*, opts, script_var_handler, try_logging_errors, util};
use anyhow::*;
use futures_util::StreamExt;
use std::{
    collections::HashMap,
    os::unix::io::AsRawFd,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc::*,
};

pub fn initialize_server(should_detach: bool, action: opts::ActionWithServer) -> Result<()> {
    if should_detach {
        do_detach()?;
    }

    simple_signal::set_handler(&[simple_signal::Signal::Int, simple_signal::Signal::Term], move |_| {
        println!("Shutting down eww daemon...");
        if let Err(e) = crate::application_lifecycle::send_exit() {
            eprintln!("Failed to send application shutdown event to workers: {:?}", e);
            std::process::exit(1);
        }
    });

    let config_file_path = crate::CONFIG_DIR.join("eww.xml");
    let config_dir = config_file_path
        .parent()
        .context("config file did not have a parent?!")?
        .to_owned();
    let scss_file_path = config_dir.join("eww.scss");

    log::info!("reading configuration from {:?}", &config_file_path);
    let eww_config = config::EwwConfig::read_from_file(&config_file_path)?;

    gtk::init()?;
    let (ui_send, mut ui_recv) = tokio::sync::mpsc::unbounded_channel();

    log::info!("Initializing script var handler");
    let script_var_handler = script_var_handler::init(ui_send.clone());

    let mut app = app::App {
        eww_state: EwwState::from_default_vars(eww_config.generate_initial_state()?),
        eww_config,
        windows: HashMap::new(),
        css_provider: gtk::CssProvider::new(),
        script_var_handler,
        app_evt_send: ui_send.clone(),
    };

    if let Some(screen) = gdk::Screen::get_default() {
        gtk::StyleContext::add_provider_for_screen(&screen, &app.css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    if let Ok(eww_css) = util::parse_scss_from_file(&scss_file_path) {
        app.load_css(&eww_css)?;
    }

    // run the command that eww was started with
    log::info!("running command: {:?}", &action);
    let (command, maybe_response_recv) = action.into_eww_command();
    app.handle_command(command);

    // initialize all the handlers and tasks running asyncronously
    init_async_part(config_file_path, scss_file_path, maybe_response_recv, ui_send);

    glib::MainContext::default().spawn_local(async move {
        while let Some(event) = ui_recv.recv().await {
            app.handle_command(event);
        }
    });

    gtk::main();
    log::info!("main application thread finished");

    Ok(())
}

fn init_async_part(
    config_file_path: PathBuf,
    scss_file_path: PathBuf,
    maybe_response_recv: Option<UnboundedReceiver<String>>,
    ui_send: UnboundedSender<app::EwwCommand>,
) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to initialize tokio runtime");
        rt.block_on(async {
            // TODO This really does not belong here at all :<
            // print out the response of this initial command, if there is any
            tokio::spawn(async {
                if let Some(mut response_recv) = maybe_response_recv {
                    if let Ok(Some(response)) = tokio::time::timeout(Duration::from_millis(100), response_recv.recv()).await {
                        println!("{}", response);
                    }
                }
            });

            let filewatch_join_handle = {
                let ui_send = ui_send.clone();
                tokio::spawn(async move { run_filewatch(config_file_path, scss_file_path, ui_send).await })
            };

            let ipc_server_join_handle = {
                let ui_send = ui_send.clone();
                tokio::spawn(async move { run_ipc_server(ui_send).await })
            };

            let forward_exit_to_app_handle = {
                let ui_send = ui_send.clone();
                tokio::spawn(async move {
                    // Wait for application exit event
                    let _ = crate::application_lifecycle::recv_exit().await;
                    // Then forward that to the application
                    let _ = ui_send.send(app::EwwCommand::KillServer);
                })
            };

            let result = tokio::try_join!(filewatch_join_handle, ipc_server_join_handle, forward_exit_to_app_handle);

            if let Err(e) = result {
                eprintln!("Eww exiting with error: {:?}", e);
            }
        })
    });
}

async fn run_ipc_server(evt_send: UnboundedSender<app::EwwCommand>) -> Result<()> {
    let listener = tokio::net::UnixListener::bind(&*crate::IPC_SOCKET_PATH)?;
    log::info!("IPC server initialized");
    crate::loop_select_exiting! {
        connection = listener.accept() => match connection {
            Ok((stream, _addr)) => {
                let evt_send = evt_send.clone();
                tokio::spawn(async move {
                    let result = handle_connection(stream, evt_send.clone()).await;
                    crate::print_result_err!("while handling IPC connection with client", result);
                });
            },
            Err(e) => eprintln!("Failed to connect to client: {:?}", e),
        }
    }
    Ok(())
}

/// Handle a single IPC connection from start to end.
async fn handle_connection(mut stream: tokio::net::UnixStream, evt_send: UnboundedSender<app::EwwCommand>) -> Result<()> {
    let (mut stream_read, mut stream_write) = stream.split();

    let action: opts::ActionWithServer = {
        let mut message_byte_length = [0u8; 4];
        stream_read
            .read_exact(&mut message_byte_length)
            .await
            .context("Failed to read message size header in IPC message")?;
        let message_byte_length = u32::from_be_bytes(message_byte_length);
        let mut raw_message = Vec::<u8>::with_capacity(message_byte_length as usize);
        while raw_message.len() < message_byte_length as usize {
            stream_read
                .read_buf(&mut raw_message)
                .await
                .context("Failed to read actual IPC message")?;
        }

        bincode::deserialize(&raw_message).context("Failed to parse client message")?
    };

    log::info!("received command from IPC: {:?}", &action);

    let (command, maybe_response_recv) = action.into_eww_command();

    evt_send.send(command)?;

    if let Some(mut response_recv) = maybe_response_recv {
        log::info!("Waiting for response for IPC client");
        if let Ok(Some(response)) = tokio::time::timeout(Duration::from_millis(100), response_recv.recv()).await {
            let result = &stream_write.write_all(response.as_bytes()).await;
            crate::print_result_err!("sending text response to ipc client", &result);
        }
    }
    stream_write.shutdown().await?;
    Ok(())
}

/// Watch configuration files for changes, sending reload events to the eww app when the files change.
async fn run_filewatch<P: AsRef<Path>>(
    config_file_path: P,
    scss_file_path: P,
    evt_send: UnboundedSender<app::EwwCommand>,
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
                        evt_send.send(app::EwwCommand::ReloadConfig(new_eww_config))?;
                } else if event.wd == scss_file_descriptor {
                        log::info!("reloading eww css file");
                        let eww_css = crate::util::parse_scss_from_file(scss_file_path.as_ref())?;
                        evt_send.send(app::EwwCommand::ReloadCss(eww_css))?;
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
