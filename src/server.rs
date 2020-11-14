use crate::{app, config, eww_state::*, opts, script_var_handler, try_logging_errors, util};
use anyhow::*;
use std::{
    collections::HashMap,
    io::Write,
    os::unix::{io::AsRawFd, net},
    path::{Path, PathBuf},
};

pub fn initialize_server(should_detach: bool, action: opts::ActionWithServer) -> Result<()> {
    let _ = std::fs::remove_file(&*crate::IPC_SOCKET_PATH);

    if should_detach {
        do_detach()?;
    }

    simple_signal::set_handler(&[simple_signal::Signal::Int, simple_signal::Signal::Term], |_| {
        println!("Shutting down eww daemon...");
        script_var_handler::script_var_process::on_application_death();
        std::process::exit(0);
    });

    // this is so ugly because of this:  https://github.com/rust-lang/rfcs/issues/372
    let (config_file_path, scss_file_path) = util::config_path().unwrap();

    log::info!("reading configuration from {:?}", &config_file_path);
    let eww_config = config::EwwConfig::read_from_file(&config_file_path)?;

    gtk::init()?;
    let (evt_send, evt_recv) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

    log::info!("Initializing script var handler");
    let script_var_handler = script_var_handler::ScriptVarHandler::new(evt_send.clone())?;

    let mut app = app::App {
        eww_state: EwwState::from_default_vars(eww_config.generate_initial_state()?),
        eww_config,
        windows: HashMap::new(),
        css_provider: gtk::CssProvider::new(),
        script_var_handler,
        app_evt_send: evt_send.clone(),
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

    // print out the response of this initial command, if there is any
    if let Some(response_recv) = maybe_response_recv {
        if let Ok(response) = response_recv.recv_timeout(std::time::Duration::from_millis(100)) {
            println!("{}", response);
        }
    }

    run_server_thread(evt_send.clone())?;
    let _hotwatch = run_filewatch_thread(&config_file_path, &scss_file_path, evt_send)?;

    evt_recv.attach(None, move |msg| {
        app.handle_command(msg);
        glib::Continue(true)
    });

    gtk::main();

    Ok(())
}

fn run_server_thread(evt_send: glib::Sender<app::EwwCommand>) -> Result<()> {
    std::thread::spawn(move || {
        let result: Result<_> = try {
            log::info!("Starting up eww server");
            let listener = net::UnixListener::bind(&*crate::IPC_SOCKET_PATH)?;
            for stream in listener.incoming() {
                try_logging_errors!("handling message from IPC client" => {
                    let mut stream = stream?;
                    let action: opts::ActionWithServer = bincode::deserialize_from(&stream)
                        .context("Failed to read or deserialize message from client")?;
                    log::info!("received command from IPC: {:?}", &action);
                    let (command, maybe_response_recv) = action.into_eww_command();
                    evt_send.send(command)?;
                    if let Some(response_recv) = maybe_response_recv {
                        if let Ok(response) = response_recv.recv_timeout(std::time::Duration::from_millis(100)) {
                            let result = &stream.write_all(response.as_bytes());
                            crate::print_result_err!("Sending text response to ipc client", &result);
                        }
                    }
                });
            }
        };
        if let Err(err) = result {
            eprintln!("error in server thread: {}", err);
            std::process::exit(1);
        }
    });
    Ok(())
}

fn run_filewatch_thread<P: AsRef<Path>>(
    config_file_path: P,
    scss_file_path: P,
    evt_send: glib::Sender<app::EwwCommand>,
) -> Result<hotwatch::Hotwatch> {
    log::info!("Initializing config file watcher");
    let mut hotwatch = hotwatch::Hotwatch::new_with_custom_delay(std::time::Duration::from_millis(500))?;

    let config_file_change_send = evt_send.clone();
    hotwatch.watch_file_changes(config_file_path, move |path| {
        try_logging_errors!("handling change of config file" => {
            log::info!("Reloading eww configuration");
            let new_eww_config = config::EwwConfig::read_from_file(path)?;
            config_file_change_send.send(app::EwwCommand::ReloadConfig(new_eww_config))?;
        });
    })?;

    let result = hotwatch.watch_file_changes(scss_file_path, move |path| {
        try_logging_errors!("handling change of scss file" =>  {
            log::info!("reloading eww css file");
            let eww_css = util::parse_scss_from_file(path)?;
            evt_send.send(app::EwwCommand::ReloadCss(eww_css))?;
        })
    });
    crate::print_result_err!("while loading CSS file for hot-reloading", &result);
    Ok(hotwatch)
}

/// detach the process from the terminal, also redirecting stdout and stderr to
/// LOG_FILE
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

#[extend::ext(pub)]
impl hotwatch::Hotwatch {
    fn watch_file_changes<P, F>(&mut self, file_path: P, callback: F) -> Result<()>
    where
        P: AsRef<Path>,
        F: 'static + Fn(PathBuf) + Send,
    {
        Ok(self.watch(file_path, move |evt| match evt {
            hotwatch::Event::Write(path) | hotwatch::Event::NoticeWrite(path) => callback(path),
            _ => {}
        })?)
    }
}
