use crate::*;
use debug_stub_derive::*;
use script_var_handler::*;
use std::collections::HashMap;
use value::VarName;

#[derive(Debug)]
pub enum EwwEvent {
    UserCommand(Opt),
    UpdateVar(VarName, PrimitiveValue),
    ReloadConfig(config::EwwConfig),
    ReloadCss(String),
}

#[derive(DebugStub)]
pub struct App {
    pub eww_state: EwwState,
    pub eww_config: config::EwwConfig,
    pub windows: HashMap<String, gtk::Window>,
    pub css_provider: gtk::CssProvider,
    pub app_evt_send: glib::Sender<EwwEvent>,
    #[debug_stub = "ScriptVarHandler(...)"]
    pub script_var_handler: ScriptVarHandler,
}

impl App {
    pub fn handle_user_command(&mut self, opts: &Opt) -> Result<()> {
        match &opts.action {
            OptAction::Update { fieldname, value } => self.update_state(fieldname.clone(), value.clone()),
            OptAction::OpenWindow { window_name } => self.open_window(&window_name)?,
            OptAction::CloseWindow { window_name } => self.close_window(&window_name)?,
            OptAction::KillServer => {
                log::info!("Received kill command, stopping server!");
                std::process::exit(0)
            }
        }

        Ok(())
    }

    pub fn handle_event(&mut self, event: EwwEvent) {
        log::debug!("Handling event: {:?}", &event);
        let result: Result<_> = try {
            match event {
                EwwEvent::UserCommand(command) => self.handle_user_command(&command)?,
                EwwEvent::UpdateVar(key, value) => self.update_state(key, value),
                EwwEvent::ReloadConfig(config) => self.reload_all_windows(config)?,
                EwwEvent::ReloadCss(css) => self.load_css(&css)?,
            }
        };
        if let Err(err) = result {
            eprintln!("Error while handling event: {:?}", err);
        }
    }

    fn update_state(&mut self, fieldname: VarName, value: PrimitiveValue) {
        self.eww_state.update_value(fieldname, value);
    }

    fn close_window(&mut self, window_name: &str) -> Result<()> {
        let window = self
            .windows
            .get(window_name)
            .context(format!("No window with name '{}' is running.", window_name))?;
        window.close();
        Ok(())
    }

    fn open_window(&mut self, window_name: &str) -> Result<()> {
        let window_def = self
            .eww_config
            .get_windows()
            .get(window_name)
            .context(format!("No window named '{}' defined", window_name))?
            .clone();

        let window = gtk::Window::new(gtk::WindowType::Popup);
        window.set_title("Eww");
        window.set_wmclass("noswallow", "noswallow");
        window.set_type_hint(gdk::WindowTypeHint::Dock);
        window.set_position(gtk::WindowPosition::Center);
        window.set_default_size(window_def.size.0, window_def.size.1);
        window.set_size_request(window_def.size.0, window_def.size.1);
        window.set_decorated(false);
        window.set_resizable(false);

        // run on_screen_changed to set the visual correctly initially.
        on_screen_changed(&window, None);
        window.connect_screen_changed(on_screen_changed);

        let empty_local_state = HashMap::new();
        let root_widget = &widgets::element_to_gtk_thing(
            &self.eww_config.get_widgets(),
            &mut self.eww_state,
            &empty_local_state,
            &window_def.widget,
        )?;
        root_widget.get_style_context().add_class(window_name);
        window.add(root_widget);

        window.show_all();

        let gdk_window = window.get_window().context("couldn't get gdk window from gtk window")?;
        gdk_window.set_override_redirect(true);
        gdk_window.move_(window_def.position.0, window_def.position.1);
        gdk_window.show();
        gdk_window.raise();
        window.set_keep_above(true);

        self.windows.insert(window_name.to_string(), window);

        Ok(())
    }

    pub fn reload_all_windows(&mut self, config: config::EwwConfig) -> Result<()> {
        // refresh script-var poll stuff
        if let Err(e) = self.script_var_handler.setup_command_poll_tasks(&config) {
            eprintln!("Error while setting up script-var commands: {:?}", e);
        }

        self.eww_config = config;
        self.eww_state.clear_callbacks();

        let windows = self.windows.clone();
        for (window_name, window) in windows {
            window.close();
            self.open_window(&window_name)?;
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
