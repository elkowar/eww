use crate::{
    config,
    config::{window_definition::WindowName, AnchorPoint, WindowStacking},
    eww_state,
    script_var_handler::*,
    value::{AttrValue, Coords, NumWithUnit, PrimitiveValue, VarName},
    widgets,
};
use anyhow::*;
use debug_stub_derive::*;
use gdk::WindowExt;
use gtk::{ContainerExt, CssProviderExt, GtkWindowExt, StyleContextExt, WidgetExt};
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub enum EwwCommand {
    NoOp,
    UpdateVars(Vec<(VarName, PrimitiveValue)>),
    ReloadConfig(config::EwwConfig),
    ReloadCss(String),
    OpenWindow {
        window_name: WindowName,
        pos: Option<Coords>,
        size: Option<Coords>,
        anchor: Option<AnchorPoint>,
    },
    CloseWindow {
        window_name: WindowName,
    },
    KillServer,
    CloseAll,
    PrintState(tokio::sync::mpsc::UnboundedSender<String>),
    PrintDebug(tokio::sync::mpsc::UnboundedSender<String>),
}

#[derive(Debug, Clone, PartialEq)]
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
    pub windows: HashMap<WindowName, EwwWindow>,
    pub css_provider: gtk::CssProvider,
    pub app_evt_send: UnboundedSender<EwwCommand>,
    #[debug_stub = "ScriptVarHandler(...)"]
    pub script_var_handler: ScriptVarHandlerHandle,
}

impl App {
    pub fn handle_command(&mut self, event: EwwCommand) {
        log::debug!("Handling event: {:?}", &event);
        let result: Result<_> = try {
            match event {
                EwwCommand::NoOp => {}
                EwwCommand::UpdateVars(mappings) => {
                    for (var_name, new_value) in mappings {
                        self.update_state(var_name, new_value)?;
                    }
                }
                EwwCommand::ReloadConfig(config) => {
                    self.reload_all_windows(config)?;
                }
                EwwCommand::ReloadCss(css) => {
                    self.load_css(&css)?;
                }
                EwwCommand::KillServer => {
                    log::info!("Received kill command, stopping server!");
                    self.script_var_handler.stop_all();
                    self.windows.drain().for_each(|(_, w)| w.close());
                    // script_var_process::on_application_death();
                    std::process::exit(0);
                }
                EwwCommand::CloseAll => {
                    log::info!("Received close command, closing all windows");
                    for (window_name, _window) in self.windows.clone() {
                        self.close_window(&window_name)?;
                    }
                }
                EwwCommand::OpenWindow {
                    window_name,
                    pos,
                    size,
                    anchor,
                } => {
                    self.open_window(&window_name, pos, size, anchor)?;
                }
                EwwCommand::CloseWindow { window_name } => {
                    self.close_window(&window_name)?;
                }
                EwwCommand::PrintState(sender) => {
                    let output = self
                        .eww_state
                        .get_variables()
                        .iter()
                        .map(|(key, value)| format!("{}: {}", key, value))
                        .join("\n");
                    sender.send(output).context("sending response from main thread")?
                }
                EwwCommand::PrintDebug(sender) => {
                    let output = format!("state: {:#?}\n\nconfig: {:#?}", &self.eww_state, &self.eww_config);
                    sender.send(output).context("sending response from main thread")?
                }
            }
        };

        crate::print_result_err!("while handling event", &result);
    }

    fn update_state(&mut self, fieldname: VarName, value: PrimitiveValue) -> Result<()> {
        self.eww_state.update_variable(fieldname, value)
    }

    fn close_window(&mut self, window_name: &WindowName) -> Result<()> {
        let window = self
            .windows
            .remove(window_name)
            .context(format!("No window with name '{}' is running.", window_name))?;

        // Stop script-var handlers for variables that where only referenced by this window
        // TODO somehow make this whole process less shit.
        let currently_used_vars = self.get_currently_used_variables().cloned().collect::<HashSet<VarName>>();

        for unused_var in self
            .eww_state
            .vars_referenced_in(window_name)
            .into_iter()
            .filter(|var| !currently_used_vars.contains(*var))
        {
            println!("stopping for {}", &unused_var);
            self.script_var_handler.stop_for_variable(unused_var.clone());
        }

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

        // remember which variables are used before opening the window, to then
        // set up the necessary handlers for the newly used variables.
        let currently_used_vars = self.get_currently_used_variables().cloned().collect::<HashSet<_>>();

        let mut window_def = self.eww_config.get_window(window_name)?.clone();
        window_def.geometry = window_def.geometry.override_if_given(anchor, pos, size);

        let root_widget = widgets::widget_use_to_gtk_widget(
            &self.eww_config.get_widgets(),
            &mut self.eww_state,
            window_name,
            &maplit::hashmap! { "window_name".into() => AttrValue::from_primitive(window_name.to_string()) },
            &window_def.widget,
        )?;
        root_widget.get_style_context().add_class(&window_name.to_string());

        let monitor_geometry = get_monitor_geometry(window_def.screen_number.unwrap_or_else(get_default_monitor_index));
        let eww_window = initialize_window(monitor_geometry, root_widget, window_def)?;

        // initialize script var handlers for variables that where not used before opening this window.
        // TODO somehow make this less shit
        let newly_used_vars = self
            .eww_state
            .vars_referenced_in(window_name)
            .into_iter()
            .filter(|x| !currently_used_vars.contains(*x))
            .collect_vec()
            .clone();

        // TODO all of the cloning above is highly ugly.... REEEEEEEEEEEEEEEEEEEEEEEEEEEEEE
        for newly_used_var in newly_used_vars {
            let value = self.eww_config.get_script_var(&newly_used_var);
            if let Some(value) = value {
                self.script_var_handler.add(value.clone());
            }
        }

        self.windows.insert(window_name.clone(), eww_window);

        Ok(())
    }

    pub fn reload_all_windows(&mut self, config: config::EwwConfig) -> Result<()> {
        log::info!("Reloading windows");
        // refresh script-var poll stuff
        self.script_var_handler.stop_all();

        self.eww_config = config;
        self.eww_state.clear_all_window_states();

        let windows = self.windows.clone();
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

    pub fn get_currently_used_variables(&self) -> impl Iterator<Item = &VarName> {
        self.eww_state.referenced_vars()
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

    window.set_title(&format!("Eww - {}", window_def.name));
    let wm_class_name = format!("eww-{}", window_def.name);
    window.set_wmclass(&wm_class_name, &wm_class_name);
    if !window_def.focusable {
        window.set_type_hint(gdk::WindowTypeHint::Dock);
    }
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

    if window_def.stacking == WindowStacking::Foreground {
        gdk_window.raise();
        window.set_keep_above(true);
    } else {
        gdk_window.lower();
        window.set_keep_below(true);
    }

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

/// get the index of the default monitor
fn get_default_monitor_index() -> i32 {
    gdk::Display::get_default()
        .expect("could not get default display")
        .get_default_screen()
        .get_primary_monitor()
}

/// Get the monitor geometry of a given monitor number
fn get_monitor_geometry(n: i32) -> gdk::Rectangle {
    gdk::Display::get_default()
        .expect("could not get default display")
        .get_default_screen()
        .get_monitor_geometry(n)
}
