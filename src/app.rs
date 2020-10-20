use crate::{
    config,
    config::{WindowName, WindowStacking},
    eww_state,
    script_var_handler::*,
    util,
    value::{Coords, NumWithUnit, PrimitiveValue, VarName},
    widgets,
};
use anyhow::*;
use crossbeam_channel;
use debug_stub_derive::*;
use gdk::WindowExt;
use gtk::{ContainerExt, CssProviderExt, GtkWindowExt, StyleContextExt, WidgetExt};
use itertools::Itertools;

use std::collections::HashMap;

#[derive(Debug)]
pub enum EwwCommand {
    UpdateVar(VarName, PrimitiveValue),
    ReloadConfig(config::EwwConfig),
    ReloadCss(String),
    OpenWindow {
        window_name: WindowName,
        pos: Option<Coords>,
        size: Option<Coords>,
    },
    CloseWindow {
        window_name: WindowName,
    },
    KillServer,
    PrintState(crossbeam_channel::Sender<String>),
    PrintDebug(crossbeam_channel::Sender<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EwwWindow {
    pub name: config::WindowName,
    pub definition: config::EwwWindowDefinition,
    pub gtk_window: gtk::Window,
}

#[derive(DebugStub)]
pub struct App {
    pub eww_state: eww_state::EwwState,
    pub eww_config: config::EwwConfig,
    pub windows: HashMap<config::WindowName, EwwWindow>,
    pub css_provider: gtk::CssProvider,
    pub app_evt_send: glib::Sender<EwwCommand>,
    #[debug_stub = "ScriptVarHandler(...)"]
    pub script_var_handler: ScriptVarHandler,
}

impl App {
    pub fn handle_command(&mut self, event: EwwCommand) {
        log::debug!("Handling event: {:?}", &event);
        let result: Result<_> = match event {
            EwwCommand::UpdateVar(key, value) => self.update_state(key, value),
            EwwCommand::ReloadConfig(config) => self.reload_all_windows(config),
            EwwCommand::ReloadCss(css) => self.load_css(&css),
            EwwCommand::KillServer => {
                log::info!("Received kill command, stopping server!");
                std::process::exit(0);
            }
            EwwCommand::OpenWindow { window_name, pos, size } => self.open_window(&window_name, pos, size),
            EwwCommand::CloseWindow { window_name } => self.close_window(&window_name),
            EwwCommand::PrintState(sender) => {
                let output = self
                    .eww_state
                    .get_variables()
                    .iter()
                    .map(|(key, value)| format!("{}: {}", key, value))
                    .join("\n");
                sender.send(output).context("sending response from main thread")
            }
            EwwCommand::PrintDebug(sender) => {
                let output = format!("state: {:#?}\n\nconfig: {:#?}", &self.eww_state, &self.eww_config);
                sender.send(output).context("sending response from main thread")
            }
        };

        util::print_result_err("while handling event", &result);
    }

    fn update_state(&mut self, fieldname: VarName, value: PrimitiveValue) -> Result<()> {
        self.eww_state.update_variable(fieldname, value)
    }

    fn close_window(&mut self, window_name: &config::WindowName) -> Result<()> {
        let window = self
            .windows
            .remove(window_name)
            .context(format!("No window with name '{}' is running.", window_name))?;
        window.gtk_window.close();
        self.eww_state.clear_window_state(window_name);

        Ok(())
    }

    fn open_window(&mut self, window_name: &config::WindowName, pos: Option<Coords>, size: Option<Coords>) -> Result<()> {
        // remove and close existing window with the same name
        let _ = self.close_window(window_name);

        let mut window_def = self
            .eww_config
            .get_windows()
            .get(window_name)
            .context(format!("No window named '{}' defined", window_name))?
            .clone();

        let display = gdk::Display::get_default().expect("could not get default display");
        let screen_number = &window_def
            .screen_number
            .unwrap_or(display.get_default_screen().get_primary_monitor());

        let monitor_geometry = display.get_default_screen().get_monitor_geometry(*screen_number);

        window_def.position = pos.unwrap_or_else(|| window_def.position);
        window_def.size = size.unwrap_or_else(|| window_def.size);

        let actual_window_rect = get_window_rectangle_in_screen(monitor_geometry, window_def.position, window_def.size);

        let window = gtk::Window::new(gtk::WindowType::Popup);
        window.set_title(&format!("Eww - {}", window_name));
        let wm_class_name = format!("eww-{}", window_name);
        window.set_wmclass(&wm_class_name, &wm_class_name);
        window.set_type_hint(gdk::WindowTypeHint::Dock);
        window.set_position(gtk::WindowPosition::Center);
        window.set_default_size(actual_window_rect.width, actual_window_rect.height);
        window.set_size_request(actual_window_rect.width, actual_window_rect.height);
        window.set_decorated(false);
        window.set_resizable(false);

        // run on_screen_changed to set the visual correctly initially.
        on_screen_changed(&window, None);
        window.connect_screen_changed(on_screen_changed);
        let mut almost_empty_local_state = HashMap::new();
        almost_empty_local_state.insert(
            VarName("window_name".to_string()),
            crate::value::AttrValue::from_primitive(window_name.to_string()),
        );
        let root_widget = &widgets::widget_use_to_gtk_widget(
            &self.eww_config.get_widgets(),
            &mut self.eww_state,
            window_name,
            &almost_empty_local_state,
            &window_def.widget,
        )?;
        root_widget.get_style_context().add_class(&window_name.to_string());
        window.add(root_widget);

        // REAL SHIT 0_o :elkowar_with_a_looking_glass:
        window.show_all();

        let gdk_window = window.get_window().context("couldn't get gdk window from gtk window")?;
        gdk_window.set_override_redirect(true);
        gdk_window.move_(actual_window_rect.x, actual_window_rect.y);
        gdk_window.show();

        if window_def.stacking == WindowStacking::Foreground {
            gdk_window.raise();
            window.set_keep_above(true);
        } else {
            gdk_window.lower();
            window.set_keep_below(true);
        }

        let eww_window = EwwWindow {
            definition: window_def,
            gtk_window: window,
            name: window_name.clone(),
        };

        self.windows.insert(window_name.clone(), eww_window);

        Ok(())
    }

    pub fn reload_all_windows(&mut self, config: config::EwwConfig) -> Result<()> {
        // refresh script-var poll stuff
        util::print_result_err(
            "while setting up script-var commands",
            &self.script_var_handler.initialize_clean(config.get_script_vars().clone()),
        );

        self.eww_config = config;
        self.eww_state.clear_all_window_states();

        let windows = self.windows.clone();
        for (window_name, window) in windows {
            let old_pos = window.definition.position;
            let old_size = window.definition.size;
            window.gtk_window.close();
            self.open_window(&window_name, Some(old_pos.into()), Some(old_size.into()))?;
        }
        Ok(())
    }

    pub fn load_css(&mut self, css: &str) -> Result<()> {
        self.css_provider.load_from_data(css.as_bytes())?;
        Ok(())
    }
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

/// Calculate the window rectangle given the configured window [`pos`] and [`size`], which might be relative to the screen size.
fn get_window_rectangle_in_screen(screen_rect: gdk::Rectangle, pos: Coords, size: Coords) -> gdk::Rectangle {
    gdk::Rectangle {
        x: match pos.x {
            NumWithUnit::Percent(n) => ((screen_rect.width as f64 / 100.0) * n as f64) as i32,
            NumWithUnit::Pixels(n) => screen_rect.x + n,
        },
        y: match pos.y {
            NumWithUnit::Percent(n) => ((screen_rect.height as f64 / 100.0) * n as f64) as i32,
            NumWithUnit::Pixels(n) => screen_rect.y + n,
        },
        width: match size.x {
            NumWithUnit::Percent(n) => ((screen_rect.width as f64 / 100.0) * n as f64) as i32,
            NumWithUnit::Pixels(n) => n,
        },
        height: match size.y {
            NumWithUnit::Percent(n) => ((screen_rect.height as f64 / 100.0) * n as f64) as i32,
            NumWithUnit::Pixels(n) => n,
        },
    }
}
