use crate::{
    config,
    daemon_response::DaemonResponseSender,
    display_backend, error_handling_ctx,
    gtk::prelude::{ContainerExt, CssProviderExt, GtkWindowExt, StyleContextExt, WidgetExt},
    script_var_handler::ScriptVarHandlerHandle,
    state::scope_graph::{ScopeGraph, ScopeGraphEvent, ScopeIndex},
    EwwPaths, *,
};
use eww_shared_util::VarName;
use itertools::Itertools;
use simplexpr::dynval::DynVal;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};
use tokio::sync::mpsc::UnboundedSender;
use yuck::{
    config::{
        script_var_definition::ScriptVarDefinition,
        window_definition::WindowDefinition,
        window_geometry::{AnchorPoint, WindowGeometry},
    },
    value::Coords,
};

#[derive(Debug)]
pub enum DaemonCommand {
    NoOp,
    UpdateVars(Vec<(VarName, DynVal)>),
    ReloadConfigAndCss(DaemonResponseSender),
    UpdateConfig(config::EwwConfig),
    UpdateCss(String),
    OpenInspector,
    OpenMany {
        windows: Vec<String>,
        should_toggle: bool,
        sender: DaemonResponseSender,
    },
    OpenWindow {
        window_name: String,
        pos: Option<Coords>,
        size: Option<Coords>,
        anchor: Option<AnchorPoint>,
        screen: Option<i32>,
        should_toggle: bool,
        sender: DaemonResponseSender,
    },
    CloseWindows {
        windows: Vec<String>,
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
    PrintWindows(DaemonResponseSender),
}

#[derive(Debug, Clone)]
pub struct EwwWindow {
    pub name: String,
    pub definition: yuck::config::window_definition::WindowDefinition,
    pub scope_index: ScopeIndex,
    pub gtk_window: gtk::Window,
}

impl EwwWindow {
    pub fn close(self) {
        self.gtk_window.close();
    }
}

pub struct App {
    pub scope_graph: Rc<RefCell<ScopeGraph>>,
    pub eww_config: config::EwwConfig,
    pub open_windows: HashMap<String, EwwWindow>,
    /// Window names that are supposed to be open, but failed.
    /// When reloading the config, these should be opened again.
    pub failed_windows: HashSet<String>,
    pub css_provider: gtk::CssProvider,

    pub app_evt_send: UnboundedSender<DaemonCommand>,
    pub script_var_handler: ScriptVarHandlerHandle,

    pub paths: EwwPaths,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("scope_graph", &*self.scope_graph.borrow())
            .field("eww_config", &self.eww_config)
            .field("open_windows", &self.open_windows)
            .field("failed_windows", &self.failed_windows)
            .field("paths", &self.paths)
            .finish()
    }
}

impl App {
    /// Handle a DaemonCommand event.
    pub fn handle_command(&mut self, event: DaemonCommand) {
        log::debug!("Handling event: {:?}", &event);
        let result: Result<_> = try {
            match event {
                DaemonCommand::NoOp => {}
                DaemonCommand::OpenInspector => {
                    gtk::Window::set_interactive_debugging(true);
                }
                DaemonCommand::UpdateVars(mappings) => {
                    for (var_name, new_value) in mappings {
                        self.update_global_state(var_name, new_value);
                    }
                }
                DaemonCommand::ReloadConfigAndCss(sender) => {
                    let mut errors = Vec::new();

                    let config_result = config::read_from_file(&self.paths.get_yuck_path());
                    if let Err(e) = config_result.and_then(|new_config| self.load_config(new_config)) {
                        errors.push(e)
                    }
                    let css_result = crate::util::parse_scss_from_file(&self.paths.get_eww_scss_path());
                    if let Err(e) = css_result.and_then(|css| self.load_css(&css)) {
                        errors.push(e)
                    }

                    sender.respond_with_error_list(errors)?;
                }
                DaemonCommand::UpdateConfig(config) => {
                    self.load_config(config)?;
                }
                DaemonCommand::UpdateCss(css) => {
                    self.load_css(&css)?;
                }
                DaemonCommand::KillServer => {
                    log::info!("Received kill command, stopping server!");
                    self.stop_application();
                    let _ = crate::application_lifecycle::send_exit();
                }
                DaemonCommand::CloseAll => {
                    log::info!("Received close command, closing all windows");
                    for (window_name, _window) in self.open_windows.clone() {
                        self.close_window(&window_name)?;
                    }
                }
                DaemonCommand::OpenMany { windows, should_toggle, sender } => {
                    let errors = windows
                        .iter()
                        .map(|w| {
                            if should_toggle && self.open_windows.contains_key(w) {
                                self.close_window(w)
                            } else {
                                self.open_window(w, None, None, None, None)
                            }
                        })
                        .filter_map(Result::err);
                    sender.respond_with_error_list(errors)?;
                }
                DaemonCommand::OpenWindow { window_name, pos, size, anchor, screen: monitor, should_toggle, sender } => {
                    let is_open = self.open_windows.contains_key(&window_name);
                    let result = if is_open {
                        if should_toggle {
                            self.close_window(&window_name)
                        } else {
                            // user should use `eww reload` to reload windows (https://github.com/elkowar/eww/issues/260)
                            Ok(())
                        }
                    } else {
                        self.open_window(&window_name, pos, size, monitor, anchor)
                    };
                    sender.respond_with_result(result)?;
                }
                DaemonCommand::CloseWindows { windows, sender } => {
                    let errors = windows.iter().map(|window| self.close_window(window)).filter_map(Result::err);
                    sender.respond_with_error_list(errors)?;
                }
                DaemonCommand::PrintState { all, sender } => {
                    let scope_graph = self.scope_graph.borrow();
                    let used_globals_names = scope_graph.currently_used_globals();
                    let output = scope_graph
                        .scope_at(scope_graph.root_index)
                        .expect("No global scope in scopegraph")
                        .data
                        .iter()
                        .filter(|(key, _)| all || used_globals_names.contains(*key))
                        .map(|(key, value)| format!("{}: {}", key, value))
                        .join("\n");
                    sender.send_success(output)?
                }
                DaemonCommand::GetVar { name, sender } => {
                    let scope_graph = &*self.scope_graph.borrow();
                    let vars = &scope_graph.scope_at(scope_graph.root_index).expect("No root scope in graph").data;
                    match vars.get(name.as_str()) {
                        Some(x) => sender.send_success(x.to_string())?,
                        None => sender.send_failure(format!("Variable not found \"{}\"", name))?,
                    }
                }
                DaemonCommand::PrintWindows(sender) => {
                    let output = self
                        .eww_config
                        .get_windows()
                        .keys()
                        .map(|window_name| {
                            let is_open = self.open_windows.contains_key(window_name);
                            format!("{}{}", if is_open { "*" } else { "" }, window_name)
                        })
                        .join("\n");
                    sender.send_success(output)?
                }
                DaemonCommand::PrintDebug(sender) => {
                    let output = format!("{:#?}", &self);
                    sender.send_success(output)?
                }
                DaemonCommand::PrintGraph(sender) => sender.send_success(self.scope_graph.borrow().visualize())?,
            }
        };

        if let Err(err) = result {
            error_handling_ctx::print_error(err);
        }
    }

    fn stop_application(&mut self) {
        self.script_var_handler.stop_all();
        for (_, window) in self.open_windows.drain() {
            window.close();
        }
        gtk::main_quit();
    }

    fn update_global_state(&mut self, fieldname: VarName, value: DynVal) {
        let result = self.scope_graph.borrow_mut().update_global_value(&fieldname, value);
        if let Err(err) = result {
            error_handling_ctx::print_error(err);
        }

        if let Ok(linked_poll_vars) = self.eww_config.get_poll_var_link(&fieldname) {
            linked_poll_vars.iter().filter_map(|name| self.eww_config.get_script_var(name).ok()).for_each(|var| {
                if let ScriptVarDefinition::Poll(poll_var) = var {
                    let scope_graph = self.scope_graph.borrow();
                    let run_while = scope_graph
                        .evaluate_simplexpr_in_scope(scope_graph.root_index, &poll_var.run_while_expr)
                        .map(|v| v.as_bool());
                    match run_while {
                        Ok(Ok(true)) => self.script_var_handler.add(var.clone()),
                        Ok(Ok(false)) => self.script_var_handler.stop_for_variable(poll_var.name.clone()),
                        Ok(Err(err)) => error_handling_ctx::print_error(anyhow!(err)),
                        Err(err) => error_handling_ctx::print_error(anyhow!(err)),
                    };
                }
            });
        }
    }

    fn close_window(&mut self, window_name: &str) -> Result<()> {
        let eww_window = self
            .open_windows
            .remove(window_name)
            .with_context(|| format!("Tried to close window named '{}', but no such window was open", window_name))?;

        self.scope_graph.borrow_mut().remove_scope(eww_window.scope_index);

        eww_window.close();

        let unused_variables = self.scope_graph.borrow().currently_unused_globals();
        for unused_var in unused_variables {
            log::debug!("stopping for {}", &unused_var);
            self.script_var_handler.stop_for_variable(unused_var.clone());
        }

        Ok(())
    }

    fn open_window(
        &mut self,
        window_name: &str,
        pos: Option<Coords>,
        size: Option<Coords>,
        monitor: Option<i32>,
        anchor: Option<AnchorPoint>,
    ) -> Result<()> {
        self.failed_windows.remove(window_name);
        log::info!("Opening window {}", window_name);

        // if an instance of this is already running, close it
        let _ = self.close_window(window_name);

        let open_result: Result<_> = try {
            let mut window_def = self.eww_config.get_window(window_name)?.clone();
            window_def.geometry = window_def.geometry.map(|x| x.override_if_given(anchor, pos, size));

            let root_index = self.scope_graph.borrow().root_index;

            let window_scope = self.scope_graph.borrow_mut().register_new_scope(
                window_name.to_string(),
                Some(root_index),
                root_index,
                HashMap::new(),
            )?;

            let root_widget = crate::widgets::build_widget::build_gtk_widget(
                &mut *self.scope_graph.borrow_mut(),
                Rc::new(self.eww_config.get_widget_definitions().clone()),
                window_scope,
                window_def.widget.clone(),
                None,
            )?;

            root_widget.style_context().add_class(&window_name.to_string());

            let monitor_geometry = get_monitor_geometry(monitor.or(window_def.monitor_number))?;

            let eww_window = initialize_window(monitor_geometry, root_widget, window_def, window_scope)?;

            // initialize script var handlers for variables that where not used before opening this window.
            // TODO maybe this could be handled by having a track_newly_used_variables function in the scope tree?
            for used_var in self.scope_graph.borrow().variables_used_in_self_or_subscopes_of(eww_window.scope_index) {
                if let Ok(script_var) = self.eww_config.get_script_var(&used_var) {
                    self.script_var_handler.add(script_var.clone());
                }
            }

            eww_window.gtk_window.connect_destroy({
                let scope_graph_sender = self.scope_graph.borrow().event_sender.clone();
                move |_| {
                    let _ = scope_graph_sender.send(ScopeGraphEvent::RemoveScope(eww_window.scope_index));
                }
            });
            self.open_windows.insert(window_name.to_string(), eww_window);
        };

        if let Err(err) = open_result {
            self.failed_windows.insert(window_name.to_string());
            Err(err).with_context(|| format!("failed to open window `{}`", window_name))
        } else {
            Ok(())
        }
    }

    /// Load the given configuration, reloading all script-vars and attempting to reopen all windows that where opened.
    pub fn load_config(&mut self, config: config::EwwConfig) -> Result<()> {
        log::info!("Reloading windows");

        self.script_var_handler.stop_all();
        self.script_var_handler = script_var_handler::init(self.app_evt_send.clone());

        log::trace!("loading config: {:#?}", config);

        self.eww_config = config;
        self.scope_graph.borrow_mut().clear(self.eww_config.generate_initial_state()?);

        let window_names: Vec<String> =
            self.open_windows.keys().cloned().chain(self.failed_windows.iter().cloned()).dedup().collect();
        for window_name in &window_names {
            self.open_window(window_name, None, None, None, None)?;
        }
        Ok(())
    }

    pub fn load_css(&mut self, css: &str) -> Result<()> {
        self.css_provider.load_from_data(css.as_bytes())?;
        Ok(())
    }
}

fn initialize_window(
    monitor_geometry: gdk::Rectangle,
    root_widget: gtk::Widget,
    window_def: WindowDefinition,
    window_scope: ScopeIndex,
) -> Result<EwwWindow> {
    let window = display_backend::initialize_window(&window_def, monitor_geometry)
        .with_context(|| format!("monitor {} is unavailable", window_def.monitor_number.unwrap()))?;

    window.set_title(&format!("Eww - {}", window_def.name));
    window.set_position(gtk::WindowPosition::None);
    window.set_gravity(gdk::Gravity::Center);

    if let Some(geometry) = window_def.geometry {
        let actual_window_rect = get_window_rectangle(geometry, monitor_geometry);
        window.set_size_request(actual_window_rect.width, actual_window_rect.height);
        window.set_default_size(actual_window_rect.width, actual_window_rect.height);
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
    {
        if let Some(geometry) = window_def.geometry {
            let _ = apply_window_position(geometry, monitor_geometry, &window);
            if window_def.backend_options.window_type != yuck::config::backend_window_options::WindowType::Normal {
                window.connect_configure_event(move |window, _| {
                    let _ = apply_window_position(geometry, monitor_geometry, window);
                    false
                });
            }
        }
        display_backend::set_xprops(&window, monitor_geometry, &window_def)?;
    }

    window.show_all();

    Ok(EwwWindow { name: window_def.name.clone(), definition: window_def, gtk_window: window, scope_index: window_scope })
}

/// Apply the provided window-positioning rules to the window.
#[cfg(feature = "x11")]
fn apply_window_position(
    mut window_geometry: WindowGeometry,
    monitor_geometry: gdk::Rectangle,
    window: &gtk::Window,
) -> Result<()> {
    let gdk_window = window.window().context("Failed to get gdk window from gtk window")?;
    window_geometry.size = Coords::from_pixels(window.size());
    let actual_window_rect = get_window_rectangle(window_geometry, monitor_geometry);

    let gdk_origin = gdk_window.origin();

    if actual_window_rect.x != gdk_origin.1 || actual_window_rect.y != gdk_origin.2 {
        gdk_window.move_(actual_window_rect.x, actual_window_rect.y);
    }

    Ok(())
}

fn on_screen_changed(window: &gtk::Window, _old_screen: Option<&gdk::Screen>) {
    let visual = window
        .screen()
        .and_then(|screen| screen.rgba_visual().filter(|_| screen.is_composited()).or_else(|| screen.system_visual()));
    window.set_visual(visual.as_ref());
}

/// Get the monitor geometry of a given monitor number, or the default if none is given
fn get_monitor_geometry(n: Option<i32>) -> Result<gdk::Rectangle> {
    #[allow(deprecated)]
    let display = gdk::Display::default().expect("could not get default display");
    let monitor = match n {
        Some(n) => display.monitor(n).with_context(|| format!("Failed to get monitor with index {}", n))?,
        None => display.primary_monitor().context("Failed to get primary monitor from GTK")?,
    };
    Ok(monitor.geometry())
}

pub fn get_window_rectangle(geometry: WindowGeometry, screen_rect: gdk::Rectangle) -> gdk::Rectangle {
    let (offset_x, offset_y) = geometry.offset.relative_to(screen_rect.width, screen_rect.height);
    let (width, height) = geometry.size.relative_to(screen_rect.width, screen_rect.height);
    let x = screen_rect.x + offset_x + geometry.anchor_point.x.alignment_to_coordinate(width, screen_rect.width);
    let y = screen_rect.y + offset_y + geometry.anchor_point.y.alignment_to_coordinate(height, screen_rect.height);
    gdk::Rectangle { x, y, width, height }
}
