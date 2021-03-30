use crate::{
    config,
    config::{window_definition::WindowName, AnchorPoint, WindowStacking},
    display_backend, eww_state,
    script_var_handler::*,
    value::{Coords, NumWithUnit, PrimitiveValue, VarName},
};
use anyhow::*;
use debug_stub_derive::*;
use gtk::{ContainerExt, CssProviderExt, GtkWindowExt, StyleContextExt, WidgetExt};
use itertools::Itertools;
use std::{collections::HashMap, path::PathBuf, ptr};
use tokio::sync::mpsc::UnboundedSender;

/// Response that the app may send as a response to a event.
/// This is used in `DaemonCommand`s that contain a response sender.
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_more::Display)]
pub enum DaemonResponse {
    Success(String),
    Failure(String),
}

impl DaemonResponse {
    pub fn is_success(&self) -> bool {
        match self {
            DaemonResponse::Success(_) => true,
            _ => false,
        }
    }

    pub fn is_failure(&self) -> bool {
        !self.is_success()
    }
}

pub type DaemonResponseSender = tokio::sync::mpsc::UnboundedSender<DaemonResponse>;
pub type DaemonResponseReceiver = tokio::sync::mpsc::UnboundedReceiver<DaemonResponse>;

#[derive(Debug)]
pub enum DaemonCommand {
    NoOp,
    UpdateVars(Vec<(VarName, PrimitiveValue)>),
    ReloadConfigAndCss(DaemonResponseSender),
    UpdateConfig(config::EwwConfig),
    UpdateCss(String),
    OpenMany {
        windows: Vec<WindowName>,
        sender: DaemonResponseSender,
    },
    OpenWindow {
        window_name: WindowName,
        pos: Option<Coords>,
        size: Option<Coords>,
        anchor: Option<AnchorPoint>,
        sender: DaemonResponseSender,
    },
    CloseWindow {
        window_name: WindowName,
        sender: DaemonResponseSender,
    },
    KillServer,
    CloseAll,
    PrintState(DaemonResponseSender),
    PrintDebug(DaemonResponseSender),
    PrintWindows(DaemonResponseSender),
}

#[derive(Debug, Clone)]
pub struct EwwWindow {
    pub name: WindowName,
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
    pub open_windows: HashMap<WindowName, EwwWindow>,
    pub css_provider: gtk::CssProvider,

    #[debug_stub = "ScriptVarHandler(...)"]
    pub app_evt_send: UnboundedSender<DaemonCommand>,
    #[debug_stub = "ScriptVarHandler(...)"]
    pub script_var_handler: ScriptVarHandlerHandle,

    pub config_file_path: PathBuf,
    pub scss_file_path: PathBuf,
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

                    let config_result =
                        config::RawEwwConfig::read_from_file(&self.config_file_path).and_then(config::EwwConfig::generate);
                    match config_result {
                        Ok(new_config) => self.handle_command(DaemonCommand::UpdateConfig(new_config)),
                        Err(e) => errors.push(e),
                    }

                    let css_result = crate::util::parse_scss_from_file(&self.scss_file_path);
                    match css_result {
                        Ok(new_css) => self.handle_command(DaemonCommand::UpdateCss(new_css)),
                        Err(e) => errors.push(e),
                    }

                    let errors = errors.into_iter().map(|e| format!("{:?}", e)).join("\n");
                    if errors.is_empty() {
                        sender.send(DaemonResponse::Success(String::new()))?;
                    } else {
                        sender.send(DaemonResponse::Failure(errors))?;
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
                    let result = windows
                        .iter()
                        .map(|w| self.open_window(w, None, None, None))
                        .collect::<Result<()>>();
                    respond_with_error(sender, result)?;
                }
                DaemonCommand::OpenWindow {
                    window_name,
                    pos,
                    size,
                    anchor,
                    sender,
                } => {
                    let result = self.open_window(&window_name, pos, size, anchor);
                    respond_with_error(sender, result)?;
                }
                DaemonCommand::CloseWindow { window_name, sender } => {
                    let result = self.close_window(&window_name);
                    respond_with_error(sender, result)?;
                }
                DaemonCommand::PrintState(sender) => {
                    let output = self
                        .eww_state
                        .get_variables()
                        .iter()
                        .map(|(key, value)| format!("{}: {}", key, value))
                        .join("\n");
                    sender
                        .send(DaemonResponse::Success(output))
                        .context("sending response from main thread")?
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
                    sender
                        .send(DaemonResponse::Success(output))
                        .context("sending response from main thread")?
                }
                DaemonCommand::PrintDebug(sender) => {
                    let output = format!("state: {:#?}\n\nconfig: {:#?}", &self.eww_state, &self.eww_config);
                    sender
                        .send(DaemonResponse::Success(output))
                        .context("sending response from main thread")?
                }
            }
        };

        crate::print_result_err!("while handling event", &result);
    }

    fn stop_application(&mut self) {
        self.script_var_handler.stop_all();
        self.open_windows.drain().for_each(|(_, w)| w.close());
        gtk::main_quit();
    }

    fn update_state(&mut self, fieldname: VarName, value: PrimitiveValue) {
        self.eww_state.update_variable(fieldname, value)
    }

    fn close_window(&mut self, window_name: &WindowName) -> Result<()> {
        for unused_var in self.variables_only_used_in(&window_name) {
            log::info!("stopping for {}", &unused_var);
            self.script_var_handler.stop_for_variable(unused_var.clone());
        }

        let window = self
            .open_windows
            .remove(window_name)
            .context(format!("No window with name '{}' is running.", window_name))?;

        window.close();
        self.eww_state.clear_window_state(window_name);

        Ok(())
    }

    fn open_window(
        &mut self,
        window_name: &WindowName,
        pos: Option<Coords>,
        size: Option<Coords>,
        anchor: Option<config::AnchorPoint>,
    ) -> Result<()> {
        // remove and close existing window with the same name
        let _ = self.close_window(window_name);

        log::info!("Opening window {}", window_name);

        let mut window_def = self.eww_config.get_window(window_name)?.clone();
        window_def.geometry = window_def.geometry.override_if_given(anchor, pos, size);

        let root_widget = window_def.widget.render(&mut self.eww_state, window_name)?;
        root_widget.get_style_context().add_class(&window_name.to_string());

        let monitor_geometry = get_monitor_geometry(window_def.screen_number.unwrap_or_else(get_default_monitor_index));
        let eww_window = initialize_window(monitor_geometry, root_widget, window_def)?;

        self.open_windows.insert(window_name.clone(), eww_window);

        // initialize script var handlers for variables that where not used before opening this window.
        // TODO somehow make this less shit
        for newly_used_var in self
            .variables_only_used_in(&window_name)
            .filter_map(|var| self.eww_config.get_script_var(&var).ok())
        {
            self.script_var_handler.add(newly_used_var.clone());
        }

        Ok(())
    }

    /// Load the given configuration, reloading all script-vars and reopening all windows that where opened.
    pub fn load_config(&mut self, config: config::EwwConfig) -> Result<()> {
        log::info!("Reloading windows");
        // refresh script-var poll stuff
        self.script_var_handler.stop_all();

        self.eww_config = config;
        self.eww_state.clear_all_window_states();

        let windows = self.open_windows.clone();
        for (window_name, window) in windows {
            window.close();
            self.open_window(&window_name, None, None, None)?;
        }
        Ok(())
    }

    pub fn load_css(&mut self, css: &str) -> Result<()> {
        self.css_provider.load_from_data(css.as_bytes())?;
        Ok(())
    }

    /// Get all variable names that are currently referenced in any of the open windows.
    pub fn get_currently_used_variables(&self) -> impl Iterator<Item = &VarName> {
        self.open_windows
            .keys()
            .flat_map(move |window_name| self.eww_state.vars_referenced_in(window_name))
    }

    /// Get all variables mapped to a list of windows they are being used in.
    pub fn currently_used_variables<'a>(&'a self) -> HashMap<&'a VarName, Vec<&'a WindowName>> {
        let mut vars: HashMap<&'a VarName, Vec<_>> = HashMap::new();
        for window_name in self.open_windows.keys() {
            for var in self.eww_state.vars_referenced_in(window_name) {
                vars.entry(var)
                    .and_modify(|l| l.push(window_name))
                    .or_insert_with(|| vec![window_name]);
            }
        }
        vars
    }

    /// Get all variables that are only used in the given window.
    pub fn variables_only_used_in<'a>(&'a self, window: &'a WindowName) -> impl Iterator<Item = &'a VarName> {
        self.currently_used_variables()
            .into_iter()
            .filter(move |(_, wins)| wins.len() == 1 && wins.contains(&window))
            .map(|(var, _)| var)
    }
}

fn initialize_window(
    monitor_geometry: gdk::Rectangle,
    root_widget: gtk::Widget,
    mut window_def: config::EwwWindowDefinition,
) -> Result<EwwWindow> {
    let actual_window_rect = window_def.geometry.get_window_rectangle(monitor_geometry);

    let window = if window_def.focusable {
        gtk::Window::new(gtk::WindowType::Toplevel)
    } else {
        gtk::Window::new(gtk::WindowType::Popup)
    };

    #[cfg(feature = "wayland")]
    {
        // Inititialising a layer shell surface
        gtk_layer_shell::init_for_window(&window);
        // Sets the monitor where the surface is shown
        match window_def.screen_number {
            Some(index)=>{
                let monitor = get_monitor(index);
                gtk_layer_shell::set_monitor(&window, &monitor);
            },
            None=>{}
        };
        gtk_layer_shell::set_keyboard_interactivity(&window, window_def.focusable);
    }
    #[cfg(feature = "x11")]
    if !window_def.focusable {
        window.set_type_hint(gdk::WindowTypeHint::Dock);
    }

    window.set_title(&format!("Eww - {}", window_def.name));
    let wm_class_name = format!("eww-{}", window_def.name);
    window.set_wmclass(&wm_class_name, &wm_class_name);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(actual_window_rect.width, actual_window_rect.height);
    window.set_size_request(actual_window_rect.width, actual_window_rect.height);
    window.set_decorated(false);
    window.set_resizable(false);

    // run on_screen_changed to set the visual correctly initially.
    on_screen_changed(&window, None);
    window.connect_screen_changed(on_screen_changed);

    window.add(&root_widget);

    // Handle the fact that the gtk window will have a different size than specified,
    // as it is sized according to how much space it's contents require.
    // This is necessary to handle different anchors correctly in case the size was wrong.
    let (gtk_window_width, gtk_window_height) = window.get_size();
    window_def.geometry.size = Coords {
        x: NumWithUnit::Pixels(gtk_window_width),
        y: NumWithUnit::Pixels(gtk_window_height),
    };
    let actual_window_rect = window_def.geometry.get_window_rectangle(monitor_geometry);

    window.show_all();

    let gdk_window = window.get_window().context("couldn't get gdk window from gtk window")?;
    gdk_window.set_override_redirect(!window_def.focusable);
    gdk_window.move_(actual_window_rect.x, actual_window_rect.y);

    #[cfg(feature = "x11")]
    // I realy think this should go in the backend
    if window_def.stacking == WindowStacking::Foreground {
        gdk_window.raise();
        window.set_keep_above(true);
    } else {
        gdk_window.lower();
        window.set_keep_below(true);
    }

    display_backend::reserve_space_for(&window, monitor_geometry, window_def.struts)?;

    Ok(EwwWindow {
        name: window_def.name.clone(),
        definition: window_def,
        gtk_window: window,
    })
}

fn on_screen_changed(window: &gtk::Window, _old_screen: Option<&gdk::Screen>) {
    let visual = window.get_screen().and_then(|screen| {
        screen
            .get_rgba_visual()
            .filter(|_| screen.is_composited())
            .or_else(|| screen.get_system_visual())
    });
    window.set_visual(visual.as_ref());
}

fn get_default_monitor_index() -> i32 {
    gdk::Display::get_default()
        .expect("could not get default display")
        .get_default_screen()
        .get_primary_monitor()
}

fn get_monitor(n: i32) -> gdk::Monitor {
    gdk::Display::get_default()
        .expect("could not get default display")
        .get_monitor(n)
        .unwrap()
}

/// Get the monitor geometry of a given monitor number
fn get_monitor_geometry(n: i32) -> gdk::Rectangle {
    gdk::Display::get_default()
        .expect("could not get default display")
        .get_default_screen()
        .get_monitor_geometry(n)
}

/// In case of an Err, send the error message to a sender.
fn respond_with_error<T>(sender: DaemonResponseSender, result: Result<T>) -> Result<()> {
    match result {
        Ok(_) => sender.send(DaemonResponse::Success(String::new())),
        Err(e) => sender.send(DaemonResponse::Failure(format!("{:?}", e))),
    }
    .context("sending response from main thread")
}
