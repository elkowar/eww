use crate::{EwwPaths, config, daemon_response::DaemonResponseSender, display_backend, error_handling_ctx, eww_state::{self, EwwState}, script_var_handler::*};
use anyhow::*;
use debug_stub_derive::*;
use eww_shared_util::VarName;
use gdk::WindowExt;
use gtk::{ContainerExt, CssProviderExt, GtkWindowExt, StyleContextExt, WidgetExt};
use itertools::Itertools;
use simplexpr::dynval::DynVal;
use std::collections::HashMap;
use tokio::sync::mpsc::UnboundedSender;
use yuck::{
    config::window_geometry::{AnchorPoint, WindowGeometry},
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
    CloseWindow {
        window_name: String,
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
                    match config_result.and_then(|new_config| self.load_config(new_config)) {
                        Ok(()) => {}
                        Err(e) => errors.push(e),
                    }

                    let css_result = crate::util::parse_scss_from_file(&self.paths.get_eww_scss_path());
                    match css_result.and_then(|css| self.load_css(&css)) {
                        Ok(()) => {}
                        Err(e) => errors.push(e),
                    }

                    let errors = errors.into_iter().map(|e| error_handling_ctx::format_error(&e)).join("\n");
                    if errors.is_empty() {
                        sender.send_success(String::new())?;
                    } else {
                        sender.send_failure(errors)?;
                    }
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
                DaemonCommand::OpenMany { windows, sender } => {
                    let result = windows.iter().try_for_each(|w| self.open_window(w, None, None, None, None));
                    respond_with_error(sender, result)?;
                }
                DaemonCommand::OpenWindow { window_name, pos, size, anchor, screen: monitor, should_toggle, sender } => {
                    let result = if should_toggle && self.open_windows.contains_key(&window_name) {
                        self.close_window(&window_name)
                    } else {
                        self.open_window(&window_name, pos, size, monitor, anchor)
                    };
                    respond_with_error(sender, result)?;
                }
                DaemonCommand::CloseWindow { window_name, sender } => {
                    let result = self.close_window(&window_name);
                    respond_with_error(sender, result)?;
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
                    let output = format!("state: {:#?}\n\nconfig: {:#?}", &self.eww_state, &self.eww_config);
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
        self.open_windows.drain().for_each(|(_, w)| w.close());
        gtk::main_quit();
    }

    fn update_state(&mut self, fieldname: VarName, value: DynVal) {
        self.eww_state.update_variable(fieldname, value)
    }

    fn close_window(&mut self, window_name: &String) -> Result<()> {
        for unused_var in self.variables_only_used_in(window_name) {
            log::debug!("stopping for {}", &unused_var);
            self.script_var_handler.stop_for_variable(unused_var.clone());
        }

        let window = self
            .open_windows
            .remove(window_name)
            .with_context(|| format!("Tried to close window named '{}', but no such window was open", window_name))?;

        window.close();
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
        log::info!("Opening window {}", window_name);

        let mut window_def = self.eww_config.get_window(window_name)?.clone();
        window_def.geometry = window_def.geometry.map(|x| x.override_if_given(anchor, pos, size));

        let root_widget =
            window_def.widget.render(&mut self.eww_state, window_name, &self.eww_config.get_widget_definitions())?;

        // once generating the root widget has succeeded
        // remove and close existing window with the same name
        let _ = self.close_window(window_name);

        root_widget.get_style_context().add_class(&window_name.to_string());

        let monitor_geometry =
            get_monitor_geometry(monitor.or(window_def.monitor_number).unwrap_or_else(get_default_monitor_index));
        let eww_window = initialize_window(monitor_geometry, root_widget, window_def)?;

        self.open_windows.insert(window_name.clone(), eww_window);

        // initialize script var handlers for variables that where not used before opening this window.
        // TODO somehow make this less shit
        for newly_used_var in self.variables_only_used_in(window_name).filter_map(|var| self.eww_config.get_script_var(var).ok())
        {
            self.script_var_handler.add(newly_used_var.clone());
        }

        Ok(())
    }

    /// Load the given configuration, reloading all script-vars and attempting to reopen all windows that where opened.
    pub fn load_config(&mut self, config: config::EwwConfig) -> Result<()> {
        log::info!("Reloading windows");
        // refresh script-var poll stuff
        self.script_var_handler.stop_all();

        self.eww_config = config;

        let new_state = EwwState::from_default_vars(self.eww_config.generate_initial_state()?);
        let old_state = std::mem::replace(&mut self.eww_state, new_state);
        for (key, value) in old_state.get_variables() {
            if self.eww_state.get_variables().contains_key(key) {
                self.eww_state.update_variable(key.clone(), value.clone())
            }
        }

        let windows = self.open_windows.clone();
        for (window_name, _) in windows {
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
                let _ = apply_window_position(geometry, monitor_geometry, &window);
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

fn get_default_monitor_index() -> i32 {
    #[allow(deprecated)]
    gdk::Display::get_default().expect("could not get default display").get_default_screen().get_primary_monitor()
}

/// Get the monitor geometry of a given monitor number
fn get_monitor_geometry(n: i32) -> gdk::Rectangle {
    #[allow(deprecated)]
    gdk::Display::get_default().expect("could not get default display").get_default_screen().get_monitor_geometry(n)
}

/// In case of an Err, send the error message to a sender.
fn respond_with_error<T>(sender: DaemonResponseSender, result: Result<T>) -> Result<()> {
    match result {
        Ok(_) => sender.send_success(String::new()),
        Err(e) => sender.send_failure(error_handling_ctx::format_error(&e)),
    }
    .context("sending response from main thread")
}

pub fn get_window_rectangle(geometry: WindowGeometry, screen_rect: gdk::Rectangle) -> gdk::Rectangle {
    let (offset_x, offset_y) = geometry.offset.relative_to(screen_rect.width, screen_rect.height);
    let (width, height) = geometry.size.relative_to(screen_rect.width, screen_rect.height);
    let x = screen_rect.x + offset_x + geometry.anchor_point.x.alignment_to_coordinate(width, screen_rect.width);
    let y = screen_rect.y + offset_y + geometry.anchor_point.y.alignment_to_coordinate(height, screen_rect.height);
    gdk::Rectangle { x, y, width, height }
}
