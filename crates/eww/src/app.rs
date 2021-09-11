use crate::{
    config, daemon_response::DaemonResponseSender, display_backend, error_handling_ctx, eww_state, script_var_handler::*,
    EwwPaths,
};
use anyhow::*;
use debug_stub_derive::*;
use eww_shared_util::VarName;
use gdk::WindowExt;
use gtk::{ContainerExt, CssProviderExt, GtkWindowExt, StyleContextExt, WidgetExt};
use itertools::Itertools;
use simplexpr::dynval::DynVal;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc::UnboundedSender;
use yuck::{
    config::{
        script_var_definition::ScriptVarDefinition,
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
    PrintDebug(DaemonResponseSender),
    PrintWindows(DaemonResponseSender),
}

#[derive(Debug, Clone)]
pub struct EwwWindow {
    pub name: String,
    pub definition: config::EwwWindowDefinition,
    pub gtk_window: gtk::Window,
}

impl EwwWindow {
    pub fn close(self) {
        self.gtk_window.close();
    }
}

#[derive(DebugStub)]
pub struct App {
    pub eww_state: eww_state::EwwState,
    pub eww_config: config::EwwConfig,
    pub open_windows: HashMap<String, EwwWindow>,
    /// Window names that are supposed to be open, but failed.
    /// When reloading the config, these should be opened again.
    pub failed_windows: HashSet<String>,
    pub css_provider: gtk::CssProvider,

    #[debug_stub = "ScriptVarHandler(...)"]
    pub app_evt_send: UnboundedSender<DaemonCommand>,
    #[debug_stub = "ScriptVarHandler(...)"]
    pub script_var_handler: ScriptVarHandlerHandle,

    pub paths: EwwPaths,
}

impl App {
    /// Handle a DaemonCommand event.
    pub fn handle_command(&mut self, event: DaemonCommand) {
        log::debug!("Handling event: {:?}", &event);
        let result: Result<_> = try {
            match event {
                DaemonCommand::NoOp => {}
                DaemonCommand::UpdateVars(mappings) => {
                    for (var_name, new_value) in mappings {
                        self.update_state(var_name, new_value);
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
                    let vars = self.eww_state.get_variables().iter();
                    let output = if all {
                        vars.map(|(key, value)| format!("{}: {}", key, value)).join("\n")
                    } else {
                        vars.filter(|(x, _)| self.eww_state.referenced_vars().any(|var| x == &var))
                            .map(|(key, value)| format!("{}: {}", key, value))
                            .join("\n")
                    };
                    sender.send_success(output)?
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

    fn update_state(&mut self, fieldname: VarName, value: DynVal) {
        self.eww_state.update_variable(fieldname.clone(), value);

        if let Ok(linked_poll_vars) = self.eww_config.get_poll_var_link(&fieldname) {
            linked_poll_vars.iter().filter_map(|name| self.eww_config.get_script_var(name).ok()).for_each(|var| {
                if let ScriptVarDefinition::Poll(poll_var) = var {
                    match poll_var.run_while_expr.eval(self.eww_state.get_variables()).map(|v| v.as_bool()) {
                        Ok(Ok(true)) => self.script_var_handler.add(var.clone()),
                        Ok(Ok(false)) => self.script_var_handler.stop_for_variable(poll_var.name.clone()),
                        Ok(Err(err)) => error_handling_ctx::print_error(anyhow!(err)),
                        Err(err) => error_handling_ctx::print_error(anyhow!(err)),
                    };
                }
            });
        }
    }

    fn close_window(&mut self, window_name: &String) -> Result<()> {
        for unused_var in self.variables_only_used_in(window_name) {
            log::debug!("stopping for {}", &unused_var);
            self.script_var_handler.stop_for_variable(unused_var.clone());
        }

        self.open_windows
            .remove(window_name)
            .with_context(|| format!("Tried to close window named '{}', but no such window was open", window_name))?
            .close();

        self.eww_state.clear_window_state(window_name);

        Ok(())
    }

    fn open_window(
        &mut self,
        window_name: &String,
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

            let root_widget =
                window_def.widget.render(&mut self.eww_state, window_name, self.eww_config.get_widget_definitions())?;

            root_widget.get_style_context().add_class(&window_name.to_string());

            let monitor_geometry = get_monitor_geometry(monitor.or(window_def.monitor_number))?;

            let eww_window = initialize_window(monitor_geometry, root_widget, window_def)?;

            self.open_windows.insert(window_name.clone(), eww_window);

            // initialize script var handlers for variables that where not used before opening this window.
            // TODO somehow make this less shit
            for newly_used_var in
                self.variables_only_used_in(window_name).filter_map(|var| self.eww_config.get_script_var(var).ok())
            {
                self.script_var_handler.add(newly_used_var.clone());
            }
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
        // TODO the reload procedure is kinda bad.
        // It should probably instead prepare a new eww_state and everything, and then swap the instances once everything has worked.

        log::info!("Reloading windows");
        // refresh script-var poll stuff
        self.script_var_handler.stop_all();

        log::trace!("loading config: {:#?}", config);

        self.eww_config = config;
        self.eww_state.clear_all_window_states();

        let window_names: Vec<String> =
            self.open_windows.keys().cloned().chain(self.failed_windows.iter().cloned()).dedup().collect();
        for window_name in &window_names {
            self.open_window(&window_name, None, None, None, None)?;
        }
        Ok(())
    }

    pub fn load_css(&mut self, css: &str) -> Result<()> {
        self.css_provider.load_from_data(css.as_bytes())?;
        Ok(())
    }

    /// Get all variable names that are currently referenced in any of the open windows.
    pub fn get_currently_used_variables(&self) -> impl Iterator<Item = &VarName> {
        self.open_windows.keys().flat_map(move |window_name| self.eww_state.vars_referenced_in(window_name))
    }

    /// Get all variables mapped to a list of windows they are being used in.
    pub fn currently_used_variables<'a>(&'a self) -> HashMap<&'a VarName, Vec<&'a String>> {
        let mut vars: HashMap<&'a VarName, Vec<_>> = HashMap::new();
        for window_name in self.open_windows.keys() {
            for var in self.eww_state.vars_referenced_in(window_name) {
                vars.entry(var).and_modify(|l| l.push(window_name)).or_insert_with(|| vec![window_name]);
            }
        }
        vars
    }

    /// Get all variables that are only used in the given window.
    pub fn variables_only_used_in<'a>(&'a self, window: &'a String) -> impl Iterator<Item = &'a VarName> {
        self.currently_used_variables()
            .into_iter()
            .filter(move |(_, wins)| wins.len() == 1 && wins.contains(&window))
            .map(|(var, _)| var)
    }
}

fn initialize_window(
    monitor_geometry: gdk::Rectangle,
    root_widget: gtk::Widget,
    window_def: config::EwwWindowDefinition,
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

    window.show_all();

    #[cfg(feature = "x11")]
    {
        if let Some(geometry) = window_def.geometry {
            let _ = apply_window_position(geometry, monitor_geometry, &window);
            window.connect_configure_event(move |window, _| {
                let _ = apply_window_position(geometry, monitor_geometry, window);
                false
            });
        }
        display_backend::set_xprops(&window, monitor_geometry, &window_def)?;
    }
    Ok(EwwWindow { name: window_def.name.clone(), definition: window_def, gtk_window: window })
}

/// Apply the provided window-positioning rules to the window.
#[cfg(feature = "x11")]
fn apply_window_position(
    mut window_geometry: WindowGeometry,
    monitor_geometry: gdk::Rectangle,
    window: &gtk::Window,
) -> Result<()> {
    let gdk_window = window.get_window().context("Failed to get gdk window from gtk window")?;
    window_geometry.size = Coords::from_pixels(window.get_size());
    let actual_window_rect = get_window_rectangle(window_geometry, monitor_geometry);
    gdk_window.move_(actual_window_rect.x, actual_window_rect.y);
    Ok(())
}

fn on_screen_changed(window: &gtk::Window, _old_screen: Option<&gdk::Screen>) {
    let visual = window
        .get_screen()
        .and_then(|screen| screen.get_rgba_visual().filter(|_| screen.is_composited()).or_else(|| screen.get_system_visual()));
    window.set_visual(visual.as_ref());
}

/// Get the monitor geometry of a given monitor number, or the default if none is given
fn get_monitor_geometry(n: Option<i32>) -> Result<gdk::Rectangle> {
    #[allow(deprecated)]
    let display = gdk::Display::get_default().expect("could not get default display");
    let monitor = match n {
        Some(n) => display.get_monitor(n).with_context(|| format!("Failed to get monitor with index {}", n))?,
        None => display.get_primary_monitor().context("Failed to get primary monitor from GTK")?,
    };
    Ok(monitor.get_geometry())
}

pub fn get_window_rectangle(geometry: WindowGeometry, screen_rect: gdk::Rectangle) -> gdk::Rectangle {
    let (offset_x, offset_y) = geometry.offset.relative_to(screen_rect.width, screen_rect.height);
    let (width, height) = geometry.size.relative_to(screen_rect.width, screen_rect.height);
    let x = screen_rect.x + offset_x + geometry.anchor_point.x.alignment_to_coordinate(width, screen_rect.width);
    let y = screen_rect.y + offset_y + geometry.anchor_point.y.alignment_to_coordinate(height, screen_rect.height);
    gdk::Rectangle { x, y, width, height }
}
