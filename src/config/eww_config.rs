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
    pub fn read_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let content = util::replace_env_var_references(std::fs::read_to_string(path)?);
        let document = roxmltree::Document::parse(&content)?;

        let result = EwwConfig::from_xml_element(XmlNode::from(document.root_element()).as_element()?.clone());
        result
    }

    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
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
                Ok((
                    WindowName::from(child.attr("name")?.to_owned()),
                    EwwWindowDefinition::from_xml_element(child)?,
                ))
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

        Ok(EwwConfig {
            widgets: definitions,
            windows,
            initial_variables,
            script_vars,
        })
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
