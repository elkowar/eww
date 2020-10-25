use anyhow::*;
use std::collections::HashMap;

use crate::{
    util,
    value::{PrimitiveValue, VarName},
};
use crate::PathBuf;

use super::{
    element::WidgetDefinition,
    xml_ext::{XmlElement, XmlNode},
    EwwWindowDefinition, ScriptVar, WindowName,
};

#[derive(Debug, Clone)]
pub struct EwwConfig {
    widgets: HashMap<String, WidgetDefinition>,
    windows: HashMap<WindowName, EwwWindowDefinition>,
    initial_variables: HashMap<VarName, PrimitiveValue>,

    // TODO make this a hashmap
    script_vars: Vec<ScriptVar>,
}

impl EwwConfig {

    // TODO: !!! There is definitely a better way to do this with a fold
    pub fn merge_includes(eww_config: EwwConfig, includes: Vec<EwwConfig>) -> Result<EwwConfig> {
        let mut eww_config = eww_config.clone();
        for config in includes {
            eww_config.widgets.extend(config.widgets);
            eww_config.windows.extend(config.windows);
            eww_config.script_vars.extend(config.script_vars);
            eww_config.initial_variables.extend(config.initial_variables);
        }
        Ok(eww_config)
    }

    pub fn read_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let content = util::replace_env_var_references(std::fs::read_to_string(path)?);
        let document = roxmltree::Document::parse(&content)?;

        let result = EwwConfig::from_xml_element(XmlNode::from(document.root_element()).as_element()?.clone());
        result
    }

    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {

        // TODO: This is not the way
        let CONFIG_DIR: std::path::PathBuf = std::env::var("XDG_CONFIG_HOME")
            .map(|v| PathBuf::from(v))
            .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap()).join(".config"))
            .join("eww");

        // !!! This doesnt seem that bad
        let includes =
            match xml.child("includes") {
                Ok(tag) => tag.child_elements()
                    .map(|child| {
                        let path = CONFIG_DIR.join(child.attr("path").unwrap());
                        EwwConfig::read_from_file(path)
                    })
                    .collect::<Result<Vec<_>>>()
                    .context("error parsing include definitions")?,
                Err(_) => {Vec::new()}
            };


        let definitions = xml
            .child("definitions")?
            .child_elements()
            .map(|child| {
                let def = WidgetDefinition::from_xml_element(child)?;
                Ok((def.name.clone(), def))
            })
            .collect::<Result<HashMap<_, _>>>()
            .context("error parsing widget definitions")?;

        let windows = xml
            .child("windows")?
            .child_elements()
            .map(|child| {
                let def = EwwWindowDefinition::from_xml_element(child)?;
                Ok((def.name.to_owned(), def))
            })
            .collect::<Result<HashMap<_, _>>>()
            .context("error parsing window definitions")?;

        let variables_block = xml.child("variables").ok();

        let mut initial_variables = HashMap::new();
        let mut script_vars = Vec::new();
        if let Some(variables_block) = variables_block {
            for node in variables_block.child_elements() {
                match node.tag_name() {
                    "var" => {
                        initial_variables.insert(
                            VarName(node.attr("name")?.to_owned()),
                            PrimitiveValue::from_string(
                                node.only_child()
                                    .map(|c| c.as_text_or_sourcecode())
                                    .unwrap_or_else(|_| String::new()),
                            ),
                        );
                    }
                    "script-var" => {
                        script_vars.push(ScriptVar::from_xml_element(node)?);
                    }
                    _ => bail!("Illegal element in variables block: {}", node.as_tag_string()),
                }
            }
        }

        // TODO: !!! Names are wacky
        let current_config = EwwConfig {
            widgets: definitions,
            windows,
            initial_variables,
            script_vars,
        };
        EwwConfig::merge_includes(current_config, includes)
    }

    // TODO this is kinda ugly
    pub fn generate_initial_state(&self) -> Result<HashMap<VarName, PrimitiveValue>> {
        let mut vars = self
            .script_vars
            .iter()
            .map(|var| Ok((var.name().clone(), var.initial_value()?)))
            .collect::<Result<HashMap<_, _>>>()?;
        vars.extend(self.get_default_vars().clone());
        Ok(vars)
    }

    pub fn get_widgets(&self) -> &HashMap<String, WidgetDefinition> {
        &self.widgets
    }

    pub fn get_windows(&self) -> &HashMap<WindowName, EwwWindowDefinition> {
        &self.windows
    }

    pub fn get_window(&self, name: &WindowName) -> Result<&EwwWindowDefinition> {
        self.windows
            .get(name)
            .with_context(|| format!("No window named '{}' exists", name))
    }

    pub fn get_default_vars(&self) -> &HashMap<VarName, PrimitiveValue> {
        &self.initial_variables
    }

    pub fn get_script_vars(&self) -> &Vec<ScriptVar> {
        &self.script_vars
    }

    pub fn get_script_var(&self, name: &VarName) -> Option<&ScriptVar> {
        self.script_vars.iter().find(|x| x.name() == name)
    }
}
