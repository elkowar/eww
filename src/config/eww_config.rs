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
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct EwwConfig {
    widgets: HashMap<String, WidgetDefinition>,
    windows: HashMap<WindowName, EwwWindowDefinition>,
    initial_variables: HashMap<VarName, PrimitiveValue>,

    // TODO make this a hashmap
    script_vars: Vec<ScriptVar>,
    pub filepath: PathBuf,
}

impl EwwConfig {
    pub fn merge_includes(mut eww_config: EwwConfig, includes: Vec<EwwConfig>) -> Result<EwwConfig> {
        // TODO issue warnings on conflict
        for config in includes {
            eww_config.widgets.extend(config.widgets);
            eww_config.windows.extend(config.windows);
            eww_config.script_vars.extend(config.script_vars);
            eww_config.initial_variables.extend(config.initial_variables);
        }
        Ok(eww_config)
    }

    pub fn read_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let result: Result<_> = try {
            let content = util::replace_env_var_references(std::fs::read_to_string(path.as_ref())?);
            let document = roxmltree::Document::parse(&content)?;
            let root_node = XmlNode::from(document.root_element());
            let root_element = root_node.as_element()?;
            EwwConfig::from_xml_element(root_element.clone(), path.as_ref())?
        };
        result.with_context(|| format!("Failed to parse xml config in {}", path.as_ref().display()))
    }

    pub fn from_xml_element<P: AsRef<std::path::Path>>(xml: XmlElement, path: P) -> Result<Self> {
        let path = path.as_ref();

        let includes = match xml.child("includes").ok() {
            Some(tag) => tag
                .child_elements()
                .map(|child| {
                    let childpath = child.attr("path")?;
                    let basepath = path.parent().unwrap();
                    EwwConfig::read_from_file(basepath.join(childpath))
                })
                .collect::<Result<Vec<_>>>()
                .context(format!("error handling include definitions at: {}", path.display()))?,
            None => Default::default(),
        };

        let definitions = match xml.child("definitions").ok() {
            Some(tag) => tag
                .child_elements()
                .map(|child| {
                    let def = WidgetDefinition::from_xml_element(child)?;
                    Ok((def.name.clone(), def))
                })
                .collect::<Result<HashMap<_, _>>>()
                .with_context(|| format!("error parsing widget definitions at: {}", path.display()))?,
            None => Default::default(),
        };

        let windows = match xml.child("windows").ok() {
            Some(tag) => tag
                .child_elements()
                .map(|child| {
                    let def = EwwWindowDefinition::from_xml_element(child)?;
                    Ok((def.name.to_owned(), def))
                })
                .collect::<Result<HashMap<_, _>>>()
                .with_context(|| format!("error parsing window definitions at: {}", path.display()))?,
            None => Default::default(),
        };

        let (initial_variables, script_vars) = match xml.child("variables").ok() {
            Some(tag) => parse_variables_block(tag)?,
            None => Default::default(),
        };

        let current_config = EwwConfig {
            widgets: definitions,
            windows,
            initial_variables,
            script_vars,
            filepath: path.to_path_buf(),
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

fn parse_variables_block(xml: XmlElement) -> Result<(HashMap<VarName, PrimitiveValue>, Vec<ScriptVar>)> {
    let mut normal_vars = HashMap::new();
    let mut script_vars = Vec::new();
    for node in xml.child_elements() {
        match node.tag_name() {
            "var" => {
                let var_name = VarName(node.attr("name")?.to_owned());
                let value = node
                    .only_child()
                    .map(|c| c.as_text_or_sourcecode())
                    .unwrap_or_else(|_| String::new());
                normal_vars.insert(var_name, PrimitiveValue::from_string(value));
            }
            "script-var" => {
                script_vars.push(ScriptVar::from_xml_element(node)?);
            }
            _ => bail!("Illegal element in variables block: {}", node.as_tag_string()),
        }
    }
    Ok((normal_vars, script_vars))
}

#[cfg(test)]
mod test {
    use crate::config::{EwwConfig, XmlNode};
    use std::collections::HashMap;

    #[test]
    fn test_merge_includes() {
        let input1 = r#"
           <eww>
              <definitions>
                <def name="test1">
                  <box orientation="v">
                    {{var1}}
                  </box>
                </def>
              </definitions>

              <variables>
                <var name="var1">var1</var>
              </variables>
              <windows>
                <window name="window1">
                  <size x="100" y="200" />
                  <pos x="100" y="200" />
                  <widget>
                    <test1 name="test2" />
                  </widget>
                </window>
              </windows>
            </eww>
        "#;
        let input2 = r#"
            <eww>
              <definitions>
                <def name="test2">
                  <box orientation="v">
                    {{var2}}
                  </box>
                </def>
              </definitions>
              <variables>
                <var name="var2">var2</var>
              </variables>
              <windows>
                <window name="window2">
                  <size x="100" y="200" />
                  <pos x="100" y="200" />
                  <widget>
                    <test2 name="test2" />
                  </widget>
                </window>
              </windows>
            </eww>
        "#;

        let document1 = roxmltree::Document::parse(&input1).unwrap();
        let document2 = roxmltree::Document::parse(input2).unwrap();
        let config1 = EwwConfig::from_xml_element(XmlNode::from(document1.root_element()).as_element().unwrap().clone(), "");
        let config2 = EwwConfig::from_xml_element(XmlNode::from(document2.root_element()).as_element().unwrap().clone(), "");
        let base_config = EwwConfig {
            widgets: HashMap::new(),
            windows: HashMap::new(),
            initial_variables: HashMap::new(),
            script_vars: Vec::new(),
            filepath: "test_path".into(),
        };

        let merged_config = EwwConfig::merge_includes(base_config, vec![config1.unwrap(), config2.unwrap()]).unwrap();

        assert_eq!(merged_config.widgets.len(), 2);
        assert_eq!(merged_config.windows.len(), 2);
        assert_eq!(merged_config.initial_variables.len(), 2);
        assert_eq!(merged_config.script_vars.len(), 0);
    }
}
