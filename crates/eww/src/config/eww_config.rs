use anyhow::*;
use eww_shared_util::VarName;
use std::{collections::HashMap, path::Path};
use yuck::config::{
    file_provider::YuckFiles, script_var_definition::ScriptVarDefinition, widget_definition::WidgetDefinition, Config,
};

use simplexpr::dynval::DynVal;

use super::{script_var, EwwWindowDefinition};

/// Eww configuration structure.
#[derive(Debug, Clone)]
pub struct EwwConfig {
    widgets: HashMap<String, WidgetDefinition>,
    windows: HashMap<String, EwwWindowDefinition>,
    initial_variables: HashMap<VarName, DynVal>,
    script_vars: HashMap<VarName, ScriptVarDefinition>,
}

impl EwwConfig {
    pub fn read_from_file(files: &mut YuckFiles, path: impl AsRef<Path>) -> Result<Self> {
        let config = Config::generate_from_main_file(files, path)?;
        let Config { widget_definitions, window_definitions, var_definitions, script_vars } = config;
        Ok(EwwConfig {
            windows: window_definitions
                .into_iter()
                .map(|(name, window)| Ok((name, EwwWindowDefinition::generate(&widget_definitions, window)?)))
                .collect::<Result<HashMap<_, _>>>()?,
            widgets: widget_definitions,
            initial_variables: var_definitions.into_iter().map(|(k, v)| (k, v.initial_value)).collect(),
            script_vars,
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

    pub fn get_windows(&self) -> &HashMap<String, EwwWindowDefinition> {
        &self.windows
    }

    pub fn get_window(&self, name: &String) -> Result<&EwwWindowDefinition> {
        self.windows.get(name).with_context(|| format!("No window named '{}' exists", name))
    }

    pub fn get_script_var(&self, name: &VarName) -> Result<&ScriptVarDefinition> {
        self.script_vars.get(name).with_context(|| format!("No script var named '{}' exists", name))
    }

    pub fn get_widget_definitions(&self) -> &HashMap<String, WidgetDefinition> {
        &self.widgets
    }
}
