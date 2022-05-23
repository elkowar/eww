use anyhow::{bail, Context, Result};
use eww_shared_util::VarName;
use std::collections::HashMap;
use yuck::{
    config::{
        file_provider::YuckFiles, script_var_definition::ScriptVarDefinition, validate::ValidationError,
        widget_definition::WidgetDefinition, window_definition::WindowDefinition, Config,
    },
    error::AstError,
};

use simplexpr::dynval::DynVal;

use crate::{config::inbuilt, error_handling_ctx, widgets::widget_definitions, EwwPaths};

use super::script_var;

/// Load an [`EwwConfig`] from the config dir of the given [`crate::EwwPaths`],
/// resetting and applying the global YuckFiles object in [`crate::error_handling_ctx`].
pub fn read_from_eww_paths(eww_paths: &EwwPaths) -> Result<EwwConfig> {
    error_handling_ctx::clear_files();
    EwwConfig::read_from_dir(&mut error_handling_ctx::YUCK_FILES.write().unwrap(), eww_paths)
}

/// Eww configuration structure.
#[derive(Debug, Clone)]
pub struct EwwConfig {
    widgets: HashMap<String, WidgetDefinition>,
    windows: HashMap<String, WindowDefinition>,
    initial_variables: HashMap<VarName, DynVal>,
    script_vars: HashMap<VarName, ScriptVarDefinition>,

    // Links variable which affect state (active/inactive) of poll var to those poll variables
    poll_var_links: HashMap<VarName, Vec<VarName>>,
}

impl Default for EwwConfig {
    fn default() -> Self {
        Self {
            widgets: HashMap::new(),
            windows: HashMap::new(),
            initial_variables: HashMap::new(),
            script_vars: HashMap::new(),
            poll_var_links: HashMap::new(),
        }
    }
}

impl EwwConfig {
    /// Load an [`EwwConfig`] from the config dir of the given [`crate::EwwPaths`], reading the main config file.
    pub fn read_from_dir(files: &mut YuckFiles, eww_paths: &EwwPaths) -> Result<Self> {
        let yuck_path = eww_paths.get_yuck_path();
        if !yuck_path.exists() {
            bail!("The configuration file `{}` does not exist", yuck_path.display());
        }
        let config = Config::generate_from_main_file(files, yuck_path)?;

        // run some validations on the configuration
        let magic_globals: Vec<_> = inbuilt::INBUILT_VAR_NAMES
            .into_iter()
            .chain(inbuilt::MAGIC_CONSTANT_NAMES)
            .into_iter()
            .map(|x| VarName::from(x.clone()))
            .collect();
        yuck::config::validate::validate(&config, magic_globals)?;

        for (name, def) in &config.widget_definitions {
            if widget_definitions::BUILTIN_WIDGET_NAMES.contains(&name.as_str()) {
                return Err(
                    AstError::ValidationError(ValidationError::AccidentalBuiltinOverride(def.span, name.to_string())).into()
                );
            }
        }

        let Config { widget_definitions, window_definitions, mut var_definitions, mut script_vars } = config;
        script_vars.extend(inbuilt::get_inbuilt_vars());
        var_definitions.extend(inbuilt::get_magic_constants(eww_paths));

        let mut poll_var_links = HashMap::<VarName, Vec<VarName>>::new();
        script_vars
            .iter()
            .filter_map(|(_, var)| if let ScriptVarDefinition::Poll(poll_var) = var { Some(poll_var) } else { None })
            .for_each(|var| {
                var.run_while_var_refs
                    .iter()
                    .for_each(|name| poll_var_links.entry(name.clone()).or_default().push(var.name.clone()))
            });

        Ok(EwwConfig {
            windows: window_definitions,
            widgets: widget_definitions,
            initial_variables: var_definitions.into_iter().map(|(k, v)| (k, v.initial_value)).collect(),
            script_vars,
            poll_var_links,
        })
    }

    // TODO this is kinda ugly
    pub fn generate_initial_state(&self) -> Result<HashMap<VarName, DynVal>> {
        let mut vars = self
            .script_vars
            .iter()
            .map(|(name, var)| Ok((name.clone(), script_var::initial_value(var)?)))
            .collect::<Result<HashMap<_, _>>>()?;
        vars.extend(self.initial_variables.clone());
        Ok(vars)
    }

    pub fn get_windows(&self) -> &HashMap<String, WindowDefinition> {
        &self.windows
    }

    pub fn get_window(&self, name: &str) -> Result<&WindowDefinition> {
        self.windows.get(name).with_context(|| {
            format!(
                "No window named '{}' exists in config.\nThis may also be caused by your config failing to load properly, \
                 please check for any other errors in that case.",
                name
            )
        })
    }

    pub fn get_script_var(&self, name: &VarName) -> Result<&ScriptVarDefinition> {
        self.script_vars.get(name).with_context(|| format!("No script var named '{}' exists", name))
    }

    pub fn get_widget_definitions(&self) -> &HashMap<String, WidgetDefinition> {
        &self.widgets
    }

    pub fn get_poll_var_link(&self, name: &VarName) -> Result<&Vec<VarName>> {
        self.poll_var_links.get(name).with_context(|| format!("{} does not links to any poll variable", name.0))
    }
}
