use anyhow::*;
use std::collections::HashMap;

use crate::{
    util,
    value::{PrimVal, VarName},
};

use super::{
    element::WidgetDefinition,
    xml_ext::{XmlElement, XmlNode},
    EwwWindowDefinition, RawEwwWindowDefinition, ScriptVar, WindowName,
};
use std::path::PathBuf;

/// Eww configuration structure.
#[derive(Debug, Clone)]
pub struct EwwConfig {
    widgets: HashMap<String, WidgetDefinition>,
    windows: HashMap<WindowName, EwwWindowDefinition>,
    initial_variables: HashMap<VarName, PrimVal>,
    script_vars: HashMap<VarName, ScriptVar>,
    pub filepath: PathBuf,
}
impl EwwConfig {
    pub fn read_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        Self::generate(RawEwwConfig::read_from_file(path)?)
    }

    pub fn generate(conf: RawEwwConfig) -> Result<Self> {
        let RawEwwConfig { windows, initial_variables, script_vars, filepath, widgets } = conf;
        Ok(EwwConfig {
            windows: windows
                .into_iter()
                .map(|(name, window)| {
                    Ok((name, EwwWindowDefinition::generate(&widgets, window).context("Failed expand window definition")?))
                })
                .collect::<Result<HashMap<_, _>>>()?,
            widgets,
            initial_variables,
            script_vars,
            filepath,
        })
    }

    // TODO this is kinda ugly
    pub fn generate_initial_state(&self) -> Result<HashMap<VarName, PrimVal>> {
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

/// Raw Eww configuration, before expanding widget usages.
#[derive(Debug, Clone)]
pub struct RawEwwConfig {
    widgets: HashMap<String, WidgetDefinition>,
    windows: HashMap<WindowName, RawEwwWindowDefinition>,
    initial_variables: HashMap<VarName, PrimVal>,
    script_vars: HashMap<VarName, ScriptVar>,
    pub filepath: PathBuf,
}

impl RawEwwConfig {
    pub fn merge_includes(mut eww_config: RawEwwConfig, includes: Vec<RawEwwConfig>) -> Result<RawEwwConfig> {
        let config_path = eww_config.filepath.clone();
        let log_conflict = |what: &str, conflict: &str, included_path: &std::path::PathBuf| {
            log::error!(
                "{} '{}' defined twice (defined in {} and in {})",
                what,
                conflict,
                config_path.display(),
                included_path.display()
            );
        };
        for included_config in includes {
            for conflict in util::extend_safe(&mut eww_config.widgets, included_config.widgets) {
                log_conflict("widget", &conflict, &included_config.filepath)
            }
            for conflict in util::extend_safe(&mut eww_config.windows, included_config.windows) {
                log_conflict("window", &conflict.to_string(), &included_config.filepath)
            }
            for conflict in util::extend_safe(&mut eww_config.script_vars, included_config.script_vars) {
                log_conflict("script-var", &conflict.to_string(), &included_config.filepath)
            }
            for conflict in util::extend_safe(&mut eww_config.initial_variables, included_config.initial_variables) {
                log_conflict("var", &conflict.to_string(), &included_config.filepath)
            }
        }
        Ok(eww_config)
    }

    pub fn read_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let result: Result<_> = try {
            let content = util::replace_env_var_references(std::fs::read_to_string(path.as_ref())?);
            let content = content.replace("&", "&amp;");
            let document = roxmltree::Document::parse(&content).map_err(|e| anyhow!(e))?;
            let root_node = XmlNode::from(document.root_element());
            let root_element = root_node.as_element()?;

            let (config, included_paths) = Self::from_xml_element(root_element.clone(), path.as_ref())
                .with_context(|| format!("Error parsing eww config file {}", path.as_ref().display()))?;

            let parsed_includes = included_paths
                .into_iter()
                .map(Self::read_from_file)
                .collect::<Result<Vec<_>>>()
                .with_context(|| format!("Included in {}", path.as_ref().display()))?;

            Self::merge_includes(config, parsed_includes)
                .context("Failed to merge included files into parent configuration file")?
        };
        result.with_context(|| format!("Failed to load eww config file {}", path.as_ref().display()))
    }

    pub fn from_xml_element<P: AsRef<std::path::Path>>(xml: XmlElement, path: P) -> Result<(Self, Vec<PathBuf>)> {
        let path = path.as_ref();

        let included_paths = match xml.child("includes").ok() {
            Some(tag) => tag
                .child_elements()
                .map(|child| {
                    crate::ensure_xml_tag_is!(child, "file");
                    Ok(util::join_path_pretty(path, PathBuf::from(child.attr("path")?)))
                })
                .collect::<Result<Vec<_>>>()?,
            None => Default::default(),
        };

        let definitions = match xml.child("definitions").ok() {
            Some(tag) => tag
                .child_elements()
                .map(|child| {
                    let def = WidgetDefinition::from_xml_element(&child).with_context(|| {
                        format!("Error parsing widget definition at {}:{}", path.display(), &child.text_pos())
                    })?;
                    Ok((def.name.clone(), def))
                })
                .collect::<Result<HashMap<_, _>>>()?,
            None => Default::default(),
        };

        let windows = match xml.child("windows").ok() {
            Some(tag) => tag
                .child_elements()
                .map(|child| {
                    let def = RawEwwWindowDefinition::from_xml_element(&child).with_context(|| {
                        format!("Error parsing window definition at {}:{}", path.display(), &child.text_pos())
                    })?;
                    Ok((def.name.to_owned(), def))
                })
                .collect::<Result<HashMap<_, _>>>()?,
            None => Default::default(),
        };

        let (initial_variables, script_vars) = match xml.child("variables").ok() {
            Some(tag) => parse_variables_block(tag)?,
            None => Default::default(),
        };

        let config = RawEwwConfig { widgets: definitions, windows, initial_variables, script_vars, filepath: path.to_path_buf() };
        Ok((config, included_paths))
    }
}

fn parse_variables_block(xml: XmlElement) -> Result<(HashMap<VarName, PrimVal>, HashMap<VarName, ScriptVar>)> {
    let mut normal_vars = HashMap::new();
    let mut script_vars = HashMap::new();
    for node in xml.child_elements() {
        match node.tag_name() {
            "var" => {
                let value = node.only_child().map(|c| c.as_text_or_sourcecode()).unwrap_or_else(|_| String::new());
                normal_vars.insert(VarName(node.attr("name")?.to_owned()), PrimVal::from_string(value));
            }
            "script-var" => {
                let script_var = ScriptVar::from_xml_element(node)?;
                script_vars.insert(script_var.name().clone(), script_var);
            }
            _ => bail!("Illegal element in variables block: {}", node.as_tag_string()),
        }
    }

    // Extends the variables with the predefined variables
    let inbuilt = crate::config::inbuilt::get_inbuilt_vars();
    for i in util::extend_safe(&mut script_vars, inbuilt) {
        log::error!(
            "script-var '{}' defined twice (defined in your config and in the eww included variables)\nHint: don't define any \
             varible like any of these: https://elkowar.github.io/eww/main/magic-variables-documenation/",
            i,
        );
    }

    Ok((normal_vars, script_vars))
}

#[cfg(test)]
mod test {
    use crate::config::{RawEwwConfig, XmlNode};
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
        let config1 =
            RawEwwConfig::from_xml_element(XmlNode::from(document1.root_element()).as_element().unwrap().clone(), "").unwrap().0;
        let config2 =
            RawEwwConfig::from_xml_element(XmlNode::from(document2.root_element()).as_element().unwrap().clone(), "").unwrap().0;
        let base_config = RawEwwConfig {
            widgets: HashMap::new(),
            windows: HashMap::new(),
            initial_variables: HashMap::new(),
            script_vars: HashMap::new(),
            filepath: "test_path".into(),
        };

        let merged_config = RawEwwConfig::merge_includes(base_config, vec![config1, config2]).unwrap();

        assert_eq!(merged_config.widgets.len(), 2);
        assert_eq!(merged_config.windows.len(), 2);
        assert_eq!(merged_config.initial_variables.len(), 2);
        assert_eq!(merged_config.script_vars.len(), 6);
    }
}
