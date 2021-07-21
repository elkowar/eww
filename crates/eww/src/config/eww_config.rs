use anyhow::*;
use std::collections::HashMap;
use yuck::{
    config::{
        script_var_definition::ScriptVarDefinition, widget_definition::WidgetDefinition, window_definition::WindowDefinition,
    },
    parser::from_ast::FromAst,
    value::VarName,
};

use simplexpr::dynval::DynVal;

use std::path::PathBuf;

/// Eww configuration structure.
#[derive(Debug, Clone)]
pub struct EwwConfig {
    widgets: HashMap<String, WidgetDefinition>,
    windows: HashMap<String, WindowDefinition>,
    initial_variables: HashMap<VarName, DynVal>,
    script_vars: HashMap<VarName, ScriptVarDefinition>,
    pub filepath: PathBuf,
}
impl EwwConfig {
    pub fn read_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let ast = yuck::parser::parse_string(0, &content)?;
        let config = yuck::config::Config::from_ast(ast)?;
        Self::generate(config)
    }

    pub fn generate(config: yuck::config::Config) -> Result<Self> {
        Ok(EwwConfig {
            windows: config
                .window_definitions
                .into_iter()
                .map(|(name, window)| {
                    Ok((
                        name,
                        WindowDefinition::generate(&config.widget_definitions, window)
                            .context("Failed expand window definition")?,
                    ))
                })
                .collect::<Result<HashMap<_, _>>>()?,
            widgets: config.widget_definitions,
            initial_variables: config.var_definitions.into_iter().map(|(k, v)| (k, v.initial_value)).collect(),
            script_vars: config.script_vars,
            filepath: todo!(),
        })
    }

    // TODO this is kinda ugly
    pub fn generate_initial_state(&self) -> Result<HashMap<VarName, DynVal>> {
        let mut vars =
            self.script_vars.iter().map(|var| Ok((var.0.clone(), var.1.initial_value()?))).collect::<Result<HashMap<_, _>>>()?;
        vars.extend(self.initial_variables.clone());
        Ok(vars)
    }

    pub fn get_windows(&self) -> &HashMap<WindowName, EwwWindowDefinition> {
        &self.windows
    }

    pub fn get_window(&self, name: &WindowName) -> Result<&EwwWindowDefinition> {
        self.windows.get(name).with_context(|| format!("No window named '{}' exists", name))
    }

    pub fn get_script_var(&self, name: &VarName) -> Result<&ScriptVar> {
        self.script_vars.get(name).with_context(|| format!("No script var named '{}' exists", name))
    }

    pub fn get_widget_definitions(&self) -> &HashMap<String, WidgetDefinition> {
        &self.widgets
    }
}

// Raw Eww configuration, before expanding widget usages.
//#[derive(Debug, Clone)]
// pub struct RawEwwConfig {
// widgets: HashMap<String, WidgetDefinition>,
// windows: HashMap<WindowName, RawEwwWindowDefinition>,
// initial_variables: HashMap<VarName, DynVal>,
// script_vars: HashMap<VarName, ScriptVar>,
// pub filepath: PathBuf,
//}

// impl RawEwwConfig {
// pub fn merge_includes(mut eww_config: RawEwwConfig, includes: Vec<RawEwwConfig>) -> Result<RawEwwConfig> {
// let config_path = eww_config.filepath.clone();
// let log_conflict = |what: &str, conflict: &str, included_path: &std::path::PathBuf| {
// log::error!(
//"{} '{}' defined twice (defined in {} and in {})",
// what,
// conflict,
// config_path.display(),
// included_path.display()
//);
//};
// for included_config in includes {
// for conflict in util::extend_safe(&mut eww_config.widgets, included_config.widgets) {
// log_conflict("widget", &conflict, &included_config.filepath)
//}
// for conflict in util::extend_safe(&mut eww_config.windows, included_config.windows) {
// log_conflict("window", &conflict.to_string(), &included_config.filepath)
//}
// for conflict in util::extend_safe(&mut eww_config.script_vars, included_config.script_vars) {
// log_conflict("script-var", &conflict.to_string(), &included_config.filepath)
//}
// for conflict in util::extend_safe(&mut eww_config.initial_variables, included_config.initial_variables) {
// log_conflict("var", &conflict.to_string(), &included_config.filepath)
//}
// Ok(eww_config)
//}
