use crate::{
    util,
    value::{PrimitiveValue, VarName},
};
use anyhow::*;
use derive_more;
use element::*;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};
use util::Coords;
use xml_ext::*;

pub mod element;
pub mod xml_ext;

#[macro_export]
macro_rules! ensure_xml_tag_is {
    ($element:ident, $name:literal) => {
        ensure!(
            $element.tag_name() == $name,
            anyhow!(
                "{} | Tag needed to be of type '{}', but was: {}",
                $element.text_pos(),
                $name,
                $element.as_tag_string()
            )
        )
    };
}

#[derive(Clone, Debug, PartialEq)]
pub struct PollScriptVar {
    pub name: VarName,
    pub command: String,
    pub interval: std::time::Duration,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TailScriptVar {
    pub name: VarName,
    pub command: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScriptVar {
    Poll(PollScriptVar),
    Tail(TailScriptVar),
}

impl ScriptVar {
    pub fn name(&self) -> &VarName {
        match self {
            ScriptVar::Poll(x) => &x.name,
            ScriptVar::Tail(x) => &x.name,
        }
    }

    pub fn initial_value(&self) -> Result<PrimitiveValue> {
        match self {
            ScriptVar::Poll(x) => Ok(crate::run_command(&x.command)?),
            ScriptVar::Tail(_) => Ok(PrimitiveValue::from_string(String::new())),
        }
    }

    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        ensure_xml_tag_is!(xml, "script-var");

        let name = VarName(xml.attr("name")?.to_owned());
        let command = xml.only_child()?.as_text()?.text();
        if let Ok(interval) = xml.attr("interval") {
            let interval = util::parse_duration(interval)?;
            Ok(ScriptVar::Poll(PollScriptVar { name, command, interval }))
        } else {
            Ok(ScriptVar::Tail(TailScriptVar { name, command }))
        }
    }
}

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
                    WindowName(child.attr("name")?.to_owned()),
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

#[repr(transparent)]
#[derive(Clone, Hash, PartialEq, Eq, derive_more::AsRef, derive_more::From, derive_more::FromStr, Serialize, Deserialize)]
pub struct WindowName(String);

impl std::borrow::Borrow<str> for WindowName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WindowName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for WindowName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WindowName(\"{}\")", self.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EwwWindowDefinition {
    pub position: Coords,
    pub size: Coords,
    pub stacking: WindowStacking,
    pub widget: WidgetUse,
}

impl EwwWindowDefinition {
    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        ensure_xml_tag_is!(xml, "window");

        let size_node = xml.child("size")?;
        let size = Coords(size_node.attr("x")?.parse()?, size_node.attr("y")?.parse()?);
        let pos_node = xml.child("pos")?;
        let position = Coords(pos_node.attr("x")?.parse()?, pos_node.attr("y")?.parse()?);

        let stacking = xml
            .attr("stacking")
            .ok()
            .map(|stacking| stacking.parse::<WindowStacking>())
            .transpose()?
            .unwrap_or_else(WindowStacking::default);

        let widget = WidgetUse::from_xml_node(xml.child("widget")?.only_child()?)?;
        Ok(EwwWindowDefinition {
            position,
            size,
            widget,
            stacking,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, derive_more::Display)]
pub enum WindowStacking {
    Foreground,
    Background,
}

impl Default for WindowStacking {
    fn default() -> Self {
        WindowStacking::Foreground
    }
}

impl std::str::FromStr for WindowStacking {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let s = s.to_lowercase();
        match s.as_str() {
            "foreground" | "fg" | "f" => Ok(WindowStacking::Foreground),
            "background" | "bg" | "b" => Ok(WindowStacking::Background),
            _ => Err(anyhow!(
                "Couldn't parse '{}' as window stacking, must be either foreground, fg, background or bg",
                s
            )),
        }
    }
}
