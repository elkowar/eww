use anyhow::*;
use std::collections::HashMap;

use crate::{
    util,
    value::{PrimitiveValue, VarName},
};

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
        let content = util::replace_env_var_references(std::fs::read_to_string(path.as_ref())?);
        let document = roxmltree::Document::parse(&content)?;

        let result = EwwConfig::from_xml_element(XmlNode::from(document.root_element()).as_element()?.clone(), path.as_ref());
        result
    }

    pub fn from_xml_element<P: AsRef<std::path::Path>>(xml: XmlElement, path: P) -> Result<Self> {
        let path= path.as_ref();
        // !!! This doesnt seem that bad
        let includes =
            match xml.child("includes") {
                Ok(tag) => tag.child_elements()
                    .map(|child| {
                        let childpath = child.attr("path")?;
                        let basepath = path.parent().unwrap();
                        EwwConfig::read_from_file(basepath.join(childpath))
                    })
                    .collect::<Result<Vec<_>>>()
                    .context(format!("error handling include definitions: {}", path.display()))?,
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
            .context(format!("error parsing widget definitions: {}", path.display()))?;

        let windows = xml
            .child("windows")?
            .child_elements()
            .map(|child| {
                Ok((
                    WindowName::from(child.attr("name")?.to_owned()),
                    EwwWindowDefinition::from_xml_element(child)?,
                ))
            })
            .collect::<Result<HashMap<_, _>>>()
            .context(format!("error parsing window definitions: {}", path.display()))?;

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

    pub fn get_default_vars(&self) -> &HashMap<VarName, PrimitiveValue> {
        &self.initial_variables
    }

    pub fn get_script_vars(&self) -> &Vec<ScriptVar> {
        &self.script_vars
    }
}
