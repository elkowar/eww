use crate::value::PrimitiveValue;
use anyhow::*;
use element::*;
use std::collections::HashMap;
use xml_ext::*;

pub mod element;
pub mod xml_ext;

#[allow(unused)]
macro_rules! try_type {
    ($typ:ty; $code:expr) => {{
        let x: $typ = try { $code };
        x
    }};
    ($typ:ty; $code:block) => {{
        let x: $typ = try { $code };
        x
    }};
}

#[derive(Debug, Clone)]
pub struct EwwConfig {
    widgets: HashMap<String, WidgetDefinition>,
    windows: HashMap<String, EwwWindowDefinition>,
    default_vars: HashMap<String, PrimitiveValue>,
}

impl EwwConfig {
    pub fn read_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let document = roxmltree::Document::parse(&content)?;
        EwwConfig::from_xml_element(XmlNode::from(document.root_element()).as_element()?)
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
            .map(|child| Ok((child.attr("name")?.to_owned(), EwwWindowDefinition::from_xml_element(child)?)))
            .collect::<Result<HashMap<_, _>>>()
            .context("error parsing window definitions")?;

        let default_vars = xml
            .child("variables")
            .ok()
            .map(|variables_node| {
                variables_node
                    .child_elements()
                    .map(|child| {
                        Ok((
                            child.tag_name().to_owned(),
                            PrimitiveValue::parse_string(&child.only_child()?.as_text()?.text()),
                        ))
                    })
                    .collect::<Result<HashMap<_, _>>>()
            })
            .transpose()
            .context("error parsing default variable value")?
            .unwrap_or_default();

        Ok(dbg!(EwwConfig {
            widgets: definitions,
            windows,
            default_vars,
        }))
    }

    pub fn get_widgets(&self) -> &HashMap<String, WidgetDefinition> {
        &self.widgets
    }
    pub fn get_windows(&self) -> &HashMap<String, EwwWindowDefinition> {
        &self.windows
    }
    pub fn get_default_vars(&self) -> &HashMap<String, PrimitiveValue> {
        &self.default_vars
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EwwWindowDefinition {
    pub position: (i32, i32),
    pub size: (i32, i32),
    pub widget: WidgetUse,
}

impl EwwWindowDefinition {
    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        if xml.tag_name() != "window" {
            bail!(
                "Only <window> tags are valid window definitions, but found {}",
                xml.as_tag_string()
            );
        }

        let size_node = xml.child("size")?;
        let size = (size_node.attr("x")?.parse()?, size_node.attr("y")?.parse()?);
        let pos_node = xml.child("pos")?;
        let position = (pos_node.attr("x")?.parse()?, pos_node.attr("y")?.parse()?);

        let widget = WidgetUse::from_xml_node(xml.child("widget")?.only_child()?)?;
        Ok(EwwWindowDefinition { position, size, widget })
    }
}
