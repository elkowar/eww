use crate::{
    daemon_response::DaemonResponseSender,
    display_backend::DisplayBackend,
    error_handling_ctx,
    gtk::prelude::{ContainerExt, CssProviderExt, GtkWindowExt, MonitorExt, StyleContextExt, WidgetExt},
    paths::EwwPaths,
    script_var_handler::ScriptVarHandlerHandle,
    state::scope_graph::{ScopeGraph, ScopeIndex},
    widgets::window::Window,
    window_arguments::WindowArguments,
    window_initiator::WindowInitiator,
    *,
};
use anyhow::anyhow;
use codespan_reporting::files::Files;
use eww_shared_util::{Span, VarName};
use gdk::Monitor;
use glib::ObjectExt;
use gtk::{gdk, glib};
use itertools::Itertools;
use once_cell::sync::Lazy;
use simplexpr::{dynval::DynVal, SimplExpr};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    marker::PhantomData,
    rc::Rc,
};
use tokio::sync::mpsc::UnboundedSender;
use yuck::{
    config::{
        monitor::MonitorIdentifier,
        script_var_definition::ScriptVarDefinition,
        window_geometry::{AnchorPoint, WindowGeometry},
    },
    error::DiagError,
    gen_diagnostic,
    value::Coords,
};

/// A command for the eww daemon.
/// While these are mostly generated from eww CLI commands (see [`opts::ActionWithServer`]),
/// they may also be generated from other places internally.
#[derive(Debug)]
pub enum DaemonCommand {
    NoOp,
    UpdateVars(Vec<(VarName, DynVal)>),
    PollVars(Vec<VarName>),
    ReloadConfigAndCss(DaemonResponseSender),
    OpenInspector,
    OpenMany {
        windows: Vec<(String, String)>,
        args: Vec<(String, VarName, DynVal)>,
        should_toggle: bool,
        sender: DaemonResponseSender,
    },
    OpenWindow {
        window_name: String,
        instance_id: Option<String>,
        pos: Option<Coords>,
        size: Option<Coords>,
        anchor: Option<AnchorPoint>,
        screen: Option<MonitorIdentifier>,
        should_toggle: bool,
        duration: Option<std::time::Duration>,
        sender: DaemonResponseSender,
        args: Option<Vec<(VarName, DynVal)>>,
    },
    CloseWindows {
        windows: Vec<String>,
        auto_reopen: bool,
        sender: DaemonResponseSender,
    },
    KillServer,
    CloseAll,
    PrintState {
        all: bool,
        sender: DaemonResponseSender,
    },
    GetVar {
        name: String,
        sender: DaemonResponseSender,
    },
    PrintDebug(DaemonResponseSender),
    PrintGraph(DaemonResponseSender),
    ListWindows(DaemonResponseSender),
    ListActiveWindows(DaemonResponseSender),
}

/// An opened window.
#[derive(Debug)]
pub struct EwwWindow {
    pub name: String,
    pub scope_index: ScopeIndex,
    pub gtk_window: Window,
    pub destroy_event_handler_id: Option<glib::SignalHandlerId>,
}

impl EwwWindow {
    /// Close the GTK window and disconnect the destroy event-handler.
    ///
    /// You need to make sure that the scope get's properly cleaned from the state graph
    /// and that script-vars get cleaned up properly
    pub fn close(self) {
        log::info!("Closing gtk window {}", self.name);
        self.gtk_window.close();
        if let Some(handler_id) = self.destroy_event_handler_id {
            self.gtk_window.disconnect(handler_id);
        }
    }
}

pub struct App<B: DisplayBackend> {
    pub scope_graph: Rc<RefCell<ScopeGraph>>,
    pub eww_config: config::EwwConfig,
    /// Map of all currently open windows to their unique IDs
    /// If no specific ID was specified whilst starting the window,
    /// it will be the same as the window name.
    /// Therefore, only one window of a given name can exist when not using IDs.
    pub open_windows: HashMap<String, EwwWindow>,
    pub instance_id_to_args: HashMap<String, WindowArguments>,
    /// Window names that are supposed to be open, but failed.
    /// When reloading the config, these should be opened again.
    pub failed_windows: HashSet<String>,
    pub css_provider: gtk::CssProvider,

    /// Sender to send [`DaemonCommand`]s
    pub app_evt_send: UnboundedSender<DaemonCommand>,
    pub script_var_handler: ScriptVarHandlerHandle,

    /// Senders that will cancel a windows auto-close timer when started with --duration.
    pub window_close_timer_abort_senders: HashMap<String, futures::channel::oneshot::Sender<()>>,

    pub paths: EwwPaths,
    pub phantom: PhantomData<B>,
}

impl<B: DisplayBackend> std::fmt::Debug for App<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("scope_graph", &*self.scope_graph.borrow())
            .field("eww_config", &self.eww_config)
            .field("open_windows", &self.open_windows)
            .field("failed_windows", &self.failed_windows)
            .field("window_arguments", &self.instance_id_to_args)
            .field("paths", &self.paths)
            .finish()
    }
}

/// Wait until the .model() is available for all monitors (or there is a timeout)
async fn wait_for_monitor_model() {
    let display = gdk::Display::default().expect("could not get default display");
    let start = std::time::Instant::now();
    loop {
        let all_monitors_set =
            (0..display.n_monitors()).all(|i| display.monitor(i).and_then(|monitor| monitor.model()).is_some());
        if all_monitors_set {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
        if std::time::Instant::now() - start > Duration::from_millis(500) {
            log::warn!("Timed out waiting for monitor model to be set");
            break;
        }
    }
}

impl<B: DisplayBackend> App<B> {
    /// Handle a [`DaemonCommand`] event, logging any errors that occur.
    pub async fn handle_command(&mut self, event: DaemonCommand) {
        if let Err(err) = self.try_handle_command(event).await {
            error_handling_ctx::print_error(err);
        }
    }

    /// Try to handle a [`DaemonCommand`] event.
    async fn try_handle_command(&mut self, event: DaemonCommand) -> Result<()> {
        log::debug!("Handling event: {:?}", &event);
        match event {
            DaemonCommand::NoOp => {}
            DaemonCommand::OpenInspector => {
                gtk::Window::set_interactive_debugging(true);
            }
            DaemonCommand::UpdateVars(mappings) => {
                for (var_name, new_value) in mappings {
                    self.update_global_variable(var_name, new_value);
                }
            }
            DaemonCommand::PollVars(names) => {
                for var_name in names {
                    self.force_poll_variable(var_name);
                }
            }
            DaemonCommand::ReloadConfigAndCss(sender) => {
                // Wait for all monitor models to be set. When a new monitor gets added, this
                // might not immediately be the case. And if we were to wait inside the
                // connect_monitor_added callback, model() never gets set. So instead we wait here.
                wait_for_monitor_model().await;
                let mut errors = Vec::new();

                let config_result = config::read_from_eww_paths(&self.paths);
                if let Err(e) = config_result.and_then(|new_config| self.load_config(new_config)) {
                    errors.push(e)
                }
                match crate::config::scss::parse_scss_from_config(self.paths.get_config_dir()) {
                    Ok((file_id, css)) => {
                        if let Err(e) = self.load_css(file_id, &css) {
                            errors.push(anyhow!(e));
                        }
                    }
                    Err(e) => {
                        errors.push(e);
                    }
                }

                sender.respond_with_error_list(errors)?;
            }
            DaemonCommand::KillServer => {
                log::info!("Received kill command, stopping server!");
                self.stop_application();
            }
            DaemonCommand::CloseAll => {
                log::info!("Received close command, closing all windows");
                for window_name in self.open_windows.keys().cloned().collect::<Vec<String>>() {
                    self.close_window(&window_name, false)?;
                }
            }
            DaemonCommand::OpenMany { windows, args, should_toggle, sender } => {
                let errors = windows
                    .iter()
                    .map(|w| {
                        let (config_name, id) = w;
                        if should_toggle && self.open_windows.contains_key(id) {
                            self.close_window(id, false)
                        } else {
                            log::debug!("Config: {}, id: {}", config_name, id);
                            let window_args = args
                                .iter()
                                .filter(|(win_id, ..)| win_id.is_empty() || win_id == id)
                                .map(|(_, n, v)| (n.clone(), v.clone()))
                                .collect();
                            self.open_window(&WindowArguments::new_from_args(id.to_string(), config_name.clone(), window_args)?)
                        }
                    })
                    .filter_map(Result::err);
                sender.respond_with_error_list(errors)?;
            }
            DaemonCommand::OpenWindow {
                window_name,
                instance_id,
                pos,
                size,
                anchor,
                screen: monitor,
                should_toggle,
                duration,
                sender,
                args,
            } => {
                let instance_id = instance_id.unwrap_or_else(|| window_name.clone());

                let is_open = self.open_windows.contains_key(&instance_id);

                let result = if should_toggle && is_open {
                    self.close_window(&instance_id, false)
                } else {
                    self.open_window(&WindowArguments {
                        instance_id,
                        window_name,
                        pos,
                        size,
                        monitor,
                        anchor,
                        duration,
                        args: args.unwrap_or_default().into_iter().collect(),
                    })
                };

                sender.respond_with_result(result)?;
            }
            DaemonCommand::CloseWindows { windows, auto_reopen, sender } => {
                let errors = windows.iter().map(|window| self.close_window(window, auto_reopen)).filter_map(Result::err);
                // Ignore sending errors, as the channel might already be closed
                let _ = sender.respond_with_error_list(errors);
            }
            DaemonCommand::PrintState { all, sender } => {
                let scope_graph = self.scope_graph.borrow();
                let used_globals_names = scope_graph.currently_used_globals();
                let output = scope_graph
                    .global_scope()
                    .data
                    .iter()
                    .filter(|(key, _)| all || used_globals_names.contains(*key))
                    .map(|(key, value)| format!("{}: {}", key, value))
                    .join("\n");
                sender.send_success(output)?
            }
            DaemonCommand::GetVar { name, sender } => {
                let scope_graph = &*self.scope_graph.borrow();
                let vars = &scope_graph.global_scope().data;
                match vars.get(name.as_str()) {
                    Some(x) => sender.send_success(x.to_string())?,
                    None => sender.send_failure(format!("Variable not found \"{}\"", name))?,
                }
            }
            DaemonCommand::ListWindows(sender) => {
                let output = self.eww_config.get_windows().keys().join("\n");
                sender.send_success(output)?
            }
            DaemonCommand::ListActiveWindows(sender) => {
                let output = self.open_windows.iter().map(|(id, window)| format!("{id}: {}", window.name)).join("\n");
                sender.send_success(output)?
            }
            DaemonCommand::PrintDebug(sender) => {
                let output = format!("{:#?}", &self);
                sender.send_success(output)?
            }
            DaemonCommand::PrintGraph(sender) => sender.send_success(self.scope_graph.borrow().visualize())?,
        }
        Ok(())
    }

    /// Fully stop eww:
    /// close all windows, stop the script_var_handler, quit the gtk appliaction and send the exit instruction to the lifecycle manager
    fn stop_application(&mut self) {
        self.script_var_handler.stop_all();
        for (_, window) in self.open_windows.drain() {
            window.close();
        }
        gtk::main_quit();
        let _ = crate::application_lifecycle::send_exit();
    }

    fn update_global_variable(&mut self, name: VarName, value: DynVal) {
        let result = self.scope_graph.borrow_mut().update_global_value(&name, value);
        if let Err(err) = result {
            error_handling_ctx::print_error(err);
        }

        self.apply_run_while_expressions_mentioning(&name);
    }

    /// Variables may be referenced in defpoll :run-while expressions.
    /// Thus, when a variable changes, the run-while conditions of all variables
    /// that mention the changed variable need to be reevaluated and reapplied.
    fn apply_run_while_expressions_mentioning(&mut self, name: &VarName) {
        let mentioning_vars = match self.eww_config.get_run_while_mentions_of(name) {
            Some(x) => x,
            None => return,
        };
        let mentioning_vars = mentioning_vars.iter().filter_map(|name| self.eww_config.get_script_var(name).ok());
        for var in mentioning_vars {
            if let ScriptVarDefinition::Poll(poll_var) = var {
                let scope_graph = self.scope_graph.borrow();
                let run_while_result = scope_graph
                    .evaluate_simplexpr_in_scope(scope_graph.root_index, &poll_var.run_while_expr)
                    .map(|v| v.as_bool());
                match run_while_result {
                    Ok(Ok(true)) => self.script_var_handler.add(var.clone()),
                    Ok(Ok(false)) => self.script_var_handler.stop_for_variable(poll_var.name.clone()),
                    Ok(Err(err)) => error_handling_ctx::print_error(anyhow!(err)),
                    Err(err) => error_handling_ctx::print_error(anyhow!(err)),
                };
            }
        }
    }

    fn force_poll_variable(&mut self, name: VarName) {
        match self.eww_config.get_script_var(&name) {
            Err(err) => error_handling_ctx::print_error(err),
            Ok(var) => {
                if let ScriptVarDefinition::Poll(poll_var) = var {
                    log::debug!("force-polling var {}", &name);
                    match script_var_handler::run_poll_once(&poll_var) {
                        Err(err) => error_handling_ctx::print_error(err),
                        Ok(value) => self.update_global_variable(name, value),
                    }
                } else {
                    error_handling_ctx::print_error(anyhow!("Script var '{}' is not polling", name))
                }
            }
        }
    }

    /// Close a window and do all the required cleanups in the scope_graph and script_var_handler
    fn close_window(&mut self, instance_id: &str, auto_reopen: bool) -> Result<()> {
        if let Some(old_abort_send) = self.window_close_timer_abort_senders.remove(instance_id) {
            _ = old_abort_send.send(());
        }
        let eww_window = self
            .open_windows
            .remove(instance_id)
            .with_context(|| format!("Tried to close window with id '{instance_id}', but no such window was open"))?;

        let scope_index = eww_window.scope_index;
        eww_window.close();

        self.scope_graph.borrow_mut().remove_scope(scope_index);

        let unused_variables = self.scope_graph.borrow().currently_unused_globals();
        for unused_var in unused_variables {
            log::debug!("stopping script-var {}", &unused_var);
            self.script_var_handler.stop_for_variable(unused_var.clone());
        }

        if auto_reopen {
            self.failed_windows.insert(instance_id.to_string());
            // There might be an alternative monitor available already, so try to re-open it immediately.
            // This can happen for example when a monitor gets disconnected and another connected,
            // and the connection event happens before the disconnect.
            if let Some(window_arguments) = self.instance_id_to_args.get(instance_id) {
                let _ = self.open_window(&window_arguments.clone());
            }
        } else {
            self.instance_id_to_args.remove(instance_id);
        }

        Ok(())
    }

    fn open_window(&mut self, window_args: &WindowArguments) -> Result<()> {
        let instance_id = &window_args.instance_id;
        self.failed_windows.remove(instance_id);
        log::info!("Opening window {} as '{}'", window_args.window_name, instance_id);

        // if an instance of this is already running, close it
        if self.open_windows.contains_key(instance_id) {
            self.close_window(instance_id, false)?;
        }

        self.instance_id_to_args.insert(instance_id.to_string(), window_args.clone());

        let open_result: Result<_> = (|| {
            let window_name: &str = &window_args.window_name;

            let window_def = self.eww_config.get_window(window_name)?.clone();
            assert_eq!(window_def.name, window_name, "window definition name did not equal the called window");

            let initiator = WindowInitiator::new(&window_def, window_args)?;

            let root_index = self.scope_graph.borrow().root_index;

            let scoped_vars_literal = initiator.get_scoped_vars().into_iter().map(|(k, v)| (k, SimplExpr::Literal(v))).collect();

            let window_scope = self.scope_graph.borrow_mut().register_new_scope(
                instance_id.to_string(),
                Some(root_index),
                root_index,
                scoped_vars_literal,
            )?;

            let root_widget = crate::widgets::build_widget::build_gtk_widget(
                &mut self.scope_graph.borrow_mut(),
                Rc::new(self.eww_config.get_widget_definitions().clone()),
                window_scope,
                window_def.widget,
                None,
            )?;

            root_widget.style_context().add_class(window_name);

            let monitor = get_gdk_monitor(initiator.monitor.clone())?;
            let mut eww_window = initialize_window::<B>(&initiator, monitor, root_widget, window_scope)?;
            eww_window.gtk_window.style_context().add_class(window_name);

            // initialize script var handlers for variables. As starting a scriptvar with the script_var_handler is idempodent,
            // we can just start script vars that are already running without causing issues
            // TODO maybe this could be handled by having a track_newly_used_variables function in the scope tree?
            for used_var in self.scope_graph.borrow().variables_used_in_self_or_subscopes_of(eww_window.scope_index) {
                if let Ok(script_var) = self.eww_config.get_script_var(&used_var) {
                    self.script_var_handler.add(script_var.clone());
                }
            }

            eww_window.destroy_event_handler_id = Some(eww_window.gtk_window.connect_destroy({
                let app_evt_sender = self.app_evt_send.clone();
                let instance_id = instance_id.to_string();
                move |_| {
                    // we don't care about the actual error response from the daemon as this is mostly just a fallback.
                    // Generally, this should get disconnected before the gtk window gets destroyed.
                    // This callback is triggered in 2 cases:
                    // - When the monitor of this window gets disconnected
                    // - When the window is closed manually.
                    // We don't distinguish here and assume the window should be reopened once a monitor
                    // becomes available again
                    let (response_sender, _) = daemon_response::create_pair();
                    let command = DaemonCommand::CloseWindows {
                        windows: vec![instance_id.clone()],
                        auto_reopen: true,
                        sender: response_sender,
                    };
                    if let Err(err) = app_evt_sender.send(command) {
                        log::error!("Error sending close window command to daemon after gtk window destroy event: {}", err);
                    }
                }
            }));

            let duration = window_args.duration;
            if let Some(duration) = duration {
                let app_evt_sender = self.app_evt_send.clone();

                let (abort_send, abort_recv) = futures::channel::oneshot::channel();

                glib::MainContext::default().spawn_local({
                    let instance_id = instance_id.to_string();
                    async move {
                        tokio::select! {
                            _ = glib::timeout_future(duration) => {
                                let (response_sender, mut response_recv) = daemon_response::create_pair();
                                let command = DaemonCommand::CloseWindows { windows: vec![instance_id.clone()], auto_reopen: false, sender: response_sender };
                                if let Err(err) = app_evt_sender.send(command) {
                                    log::error!("Error sending close window command to daemon after gtk window destroy event: {}", err);
                                }
                                _ = response_recv.recv().await;
                            }
                            _ = abort_recv => {}
                        }
                    }
                });

                if let Some(old_abort_send) = self.window_close_timer_abort_senders.insert(instance_id.to_string(), abort_send) {
                    _ = old_abort_send.send(());
                }
            }

            self.open_windows.insert(instance_id.to_string(), eww_window);
            Ok(())
        })();

        if let Err(err) = open_result {
            self.failed_windows.insert(instance_id.to_string());
            Err(err).with_context(|| format!("failed to open window `{}`", instance_id))
        } else {
            Ok(())
        }
    }

    /// Load the given configuration, reloading all script-vars and attempting to reopen all windows that where opened.
    pub fn load_config(&mut self, config: config::EwwConfig) -> Result<()> {
        log::info!("Reloading windows");

        self.script_var_handler.stop_all();
        let old_handler = std::mem::replace(&mut self.script_var_handler, script_var_handler::init(self.app_evt_send.clone()));
        old_handler.join_thread();

        log::trace!("loading config: {:#?}", config);

        self.eww_config = config;
        self.scope_graph.borrow_mut().clear(self.eww_config.generate_initial_state()?);

        let open_window_ids: Vec<String> =
            self.open_windows.keys().cloned().chain(self.failed_windows.iter().cloned()).dedup().collect();
        for instance_id in &open_window_ids {
            let window_arguments = self.instance_id_to_args.get(instance_id).with_context(|| {
                format!("Cannot reopen window, initial parameters were not saved correctly for {instance_id}")
            })?;
            self.open_window(&window_arguments.clone())?;
        }
        Ok(())
    }

    /// Load a given CSS string into the gtk css provider, returning a nicely formatted [`DiagError`] when GTK errors out
    pub fn load_css(&mut self, file_id: usize, css: &str) -> Result<()> {
        if let Err(err) = self.css_provider.load_from_data(css.as_bytes()) {
            static PATTERN: Lazy<regex::Regex> = Lazy::new(|| regex::Regex::new(r"[^:]*:(\d+):(\d+)(.*)$").unwrap());
            let nice_error_option: Option<_> = (|| {
                let captures = PATTERN.captures(err.message())?;
                let line = captures.get(1).unwrap().as_str().parse::<usize>().ok()?;
                let msg = captures.get(3).unwrap().as_str();
                let db = error_handling_ctx::FILE_DATABASE.read().ok()?;
                let line_range = db.line_range(file_id, line - 1).ok()?;
                let span = Span(line_range.start, line_range.end - 1, file_id);
                Some(DiagError(gen_diagnostic!(msg, span)))
            })();
            match nice_error_option {
                Some(error) => Err(anyhow!(error)),
                None => Err(anyhow!("CSS error: {}", err.message())),
            }
        } else {
            Ok(())
        }
    }
}

fn initialize_window<B: DisplayBackend>(
    window_init: &WindowInitiator,
    monitor: Monitor,
    root_widget: gtk::Widget,
    window_scope: ScopeIndex,
) -> Result<EwwWindow> {
    let monitor_geometry = monitor.geometry();
    let (actual_window_rect, x, y) = match window_init.geometry {
        Some(geometry) => {
            let rect = get_window_rectangle(geometry, monitor_geometry);
            (Some(rect), rect.x(), rect.y())
        }
        _ => (None, 0, 0),
    };
    let window = B::initialize_window(window_init, monitor_geometry, x, y)
        .with_context(|| format!("monitor {} is unavailable", window_init.monitor.clone().unwrap()))?;

    window.set_title(&format!("Eww - {}", window_init.name));
    window.set_position(gtk::WindowPosition::None);
    window.set_gravity(gdk::Gravity::Center);

    if let Some(actual_window_rect) = actual_window_rect {
        window.set_size_request(actual_window_rect.width(), actual_window_rect.height());
        window.set_default_size(actual_window_rect.width(), actual_window_rect.height());
    }
    window.set_decorated(false);
    window.set_skip_taskbar_hint(true);
    window.set_skip_pager_hint(true);

    // run on_screen_changed to set the visual correctly initially.
    on_screen_changed(&window, None);
    window.connect_screen_changed(on_screen_changed);

    window.add(&root_widget);

    window.realize();

    #[cfg(feature = "x11")]
    if B::IS_X11 {
        if let Some(geometry) = window_init.geometry {
            let _ = apply_window_position(geometry, monitor_geometry, &window);
            if window_init.backend_options.x11.window_type != yuck::config::backend_window_options::X11WindowType::Normal {
                window.connect_configure_event(move |window, _| {
                    let _ = apply_window_position(geometry, monitor_geometry, window);
                    false
                });
            }
        }
        display_backend::set_xprops(&window, monitor, window_init)?;
    }

    window.show_all();

    Ok(EwwWindow {
        name: window_init.name.clone(),
        gtk_window: window,
        scope_index: window_scope,
        destroy_event_handler_id: None,
    })
}

/// Apply the provided window-positioning rules to the window.
#[cfg(feature = "x11")]
fn apply_window_position(mut window_geometry: WindowGeometry, monitor_geometry: gdk::Rectangle, window: &Window) -> Result<()> {
    let gdk_window = window.window().context("Failed to get gdk window from gtk window")?;
    window_geometry.size = Coords::from_pixels(window.size());
    let actual_window_rect = get_window_rectangle(window_geometry, monitor_geometry);

    let gdk_origin = gdk_window.origin();

    if actual_window_rect.x() != gdk_origin.1 || actual_window_rect.y() != gdk_origin.2 {
        gdk_window.move_(actual_window_rect.x(), actual_window_rect.y());
    }

    Ok(())
}

fn on_screen_changed(window: &Window, _old_screen: Option<&gdk::Screen>) {
    let visual = gtk::prelude::GtkWindowExt::screen(window)
        .and_then(|screen| screen.rgba_visual().filter(|_| screen.is_composited()).or_else(|| screen.system_visual()));
    window.set_visual(visual.as_ref());
}

/// Get the monitor geometry of a given monitor, or the default if none is given
fn get_gdk_monitor(identifier: Option<MonitorIdentifier>) -> Result<Monitor> {
    let display = gdk::Display::default().expect("could not get default display");
    let monitor = match identifier {
        Some(ident) => {
            let mon = get_monitor_from_display(&display, &ident);
            mon.with_context(|| {
                let head = format!("Failed to get monitor {}\nThe available monitors are:", ident);
                let mut body = String::new();
                for m in 0..display.n_monitors() {
                    if let Some(model) = display.monitor(m).and_then(|x| x.model()) {
                        body.push_str(format!("\n\t[{}] {}", m, model).as_str());
                    }
                }
                format!("{}{}", head, body)
            })?
        }
        None => display
            .primary_monitor()
            .context("Failed to get primary monitor from GTK. Try explicitly specifying the monitor on your window.")?,
    };
    Ok(monitor)
}

/// Get the name of monitor plug for given monitor number
/// workaround gdk not providing this information on wayland in regular calls
/// gdk_screen_get_monitor_plug_name is deprecated but works fine for that case
fn get_monitor_plug_name(display: &gdk::Display, monitor_num: i32) -> Option<&str> {
    unsafe {
        use glib::translate::ToGlibPtr;
        let plug_name_pointer = gdk_sys::gdk_screen_get_monitor_plug_name(display.default_screen().to_glib_none().0, monitor_num);
        use std::ffi::CStr;
        CStr::from_ptr(plug_name_pointer).to_str().ok()
    }
}

/// Returns the [Monitor][gdk::Monitor] structure corresponding to the identifer.
/// Outside of x11, only [MonitorIdentifier::Numeric] is supported
pub fn get_monitor_from_display(display: &gdk::Display, identifier: &MonitorIdentifier) -> Option<gdk::Monitor> {
    match identifier {
        MonitorIdentifier::List(list) => {
            for ident in list {
                if let Some(monitor) = get_monitor_from_display(display, ident) {
                    return Some(monitor);
                }
            }
            None
        }
        MonitorIdentifier::Primary => display.primary_monitor(),
        MonitorIdentifier::Numeric(num) => display.monitor(*num),
        MonitorIdentifier::Name(name) => {
            for m in 0..display.n_monitors() {
                if let Some(model) = display.monitor(m).and_then(|x| x.model()) {
                    if model == *name || Some(name.as_str()) == get_monitor_plug_name(display, m) {
                        return display.monitor(m);
                    }
                }
            }
            None
        }
    }
}

pub fn get_window_rectangle(geometry: WindowGeometry, screen_rect: gdk::Rectangle) -> gdk::Rectangle {
    let (offset_x, offset_y) = geometry.offset.relative_to(screen_rect.width(), screen_rect.height());
    let (width, height) = geometry.size.relative_to(screen_rect.width(), screen_rect.height());
    let x = screen_rect.x() + offset_x + geometry.anchor_point.x.alignment_to_coordinate(width, screen_rect.width());
    let y = screen_rect.y() + offset_y + geometry.anchor_point.y.alignment_to_coordinate(height, screen_rect.height());
    gdk::Rectangle::new(x, y, width, height)
}
