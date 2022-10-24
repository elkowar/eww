use anyhow::{bail, Context, Result};
use eww_shared_util::VarName;
use std::collections::HashMap;
use yuck::{
    config::{
        script_var_definition::ScriptVarDefinition, validate::ValidationError, widget_definition::WidgetDefinition,
        window_definition::WindowDefinition, Config,
    },
    error::DiagError,
    format_diagnostic::ToDiagnostic,
};

use simplexpr::dynval::DynVal;

use crate::{config::inbuilt, error_handling_ctx, file_database::FileDatabase, paths::EwwPaths, widgets::widget_definitions};

use super::script_var;

/// Load an [`EwwConfig`] from the config dir of the given [`crate::EwwPaths`],
/// resetting and applying the global YuckFiles object in [`crate::error_handling_ctx`].
pub fn read_from_eww_paths(eww_paths: &EwwPaths) -> Result<EwwConfig> {
    error_handling_ctx::clear_files();
    EwwConfig::read_from_dir(&mut error_handling_ctx::FILE_DATABASE.write().unwrap(), eww_paths)
}

/// Eww configuration structure.
#[derive(Debug, Clone, Default)]
pub struct EwwConfig {
    widgets: HashMap<String, WidgetDefinition>,
    windows: HashMap<String, WindowDefinition>,
    initial_variables: HashMap<VarName, DynVal>,
    script_vars: HashMap<VarName, ScriptVarDefinition>,

    // map of variables to all pollvars which refer to them in their run-while-expression
    run_while_mentions: HashMap<VarName, Vec<VarName>>,
}

impl EwwConfig {
    /// Load an [`EwwConfig`] from the config dir of the given [`crate::EwwPaths`], reading the main config file.
    pub fn read_from_dir(files: &mut FileDatabase, eww_paths: &EwwPaths) -> Result<Self> {
        let yuck_path = eww_paths.get_yuck_path();
        if !yuck_path.exists() {
            bail!("The configuration file `{}` does not exist", yuck_path.display());
        }
        let config = Config::generate_from_main_file(files, yuck_path)?;

        // run some validations on the configuration
        let magic_globals: Vec<_> = inbuilt::INBUILT_VAR_NAMES
            .iter()
            .chain(inbuilt::MAGIC_CONSTANT_NAMES)
            .into_iter()
            .map(|x| VarName::from(*x))
            .collect();
        yuck::config::validate::validate(&config, magic_globals)?;

        for (name, def) in &config.widget_definitions {
            if widget_definitions::BUILTIN_WIDGET_NAMES.contains(&name.as_str()) {
                return Err(
                    DiagError(ValidationError::AccidentalBuiltinOverride(def.span, name.to_string()).to_diagnostic()).into()
                );
            }
        }

        let Config { widget_definitions, window_definitions, mut var_definitions, mut script_vars } = config;
        script_vars.extend(inbuilt::get_inbuilt_vars());
        var_definitions.extend(inbuilt::get_magic_constants(eww_paths));

        let mut run_while_mentions = HashMap::<VarName, Vec<VarName>>::new();
        for var in script_vars.values() {
            if let ScriptVarDefinition::Poll(var) = var {
                for name in var.run_while_expr.collect_var_refs() {
                    run_while_mentions.entry(name.clone()).or_default().push(var.name.clone())
                }
            }
        }

        Ok(EwwConfig {
            windows: window_definitions,
            widgets: widget_definitions,
            initial_variables: var_definitions.into_iter().map(|(k, v)| (k, v.initial_value)).collect(),
            script_vars,
            run_while_mentions,
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

    /// Given a variable name, get the names of all variables that reference that variable in their run-while (active/inactive) state
    pub fn get_run_while_mentions_of(&self, name: &VarName) -> Option<&Vec<VarName>> {
        self.run_while_mentions.get(name)
    }
}
