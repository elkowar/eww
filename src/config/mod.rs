use crate::value::PrimitiveValue;
use anyhow::*;
use element::*;
use hocon::*;
use hocon_ext::HoconExt;
use std::collections::HashMap;
use std::convert::TryFrom;
use xml_ext::*;

pub mod element;
pub mod hocon_ext;
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

    pub fn from_hocon(hocon: &Hocon) -> Result<Self> {
        let data = hocon.as_hash()?;

        let widgets = data
            .get("widgets")
            .context("widgets field missing")?
            .as_hash()?
            .iter()
            .map(|(n, def)| Ok((n.clone(), WidgetDefinition::parse_hocon(n.clone(), def)?)))
            .collect::<Result<_>>()?;

        let windows = data
            .get("windows")
            .context("windows field missing")?
            .as_hash()?
            .iter()
            .map(|(name, def)| Ok((name.clone(), EwwWindowDefinition::from_hocon(def)?)))
            .collect::<Result<_>>()?;

        let default_vars = data
            .get("default_vars")
            .unwrap_or(&Hocon::Hash(HashMap::new()))
            .as_hash()?
            .iter()
            .map(|(name, def)| Ok((name.clone(), PrimitiveValue::try_from(def)?)))
            .collect::<Result<_>>()?;

        Ok(EwwConfig {
            widgets,
            windows,
            default_vars,
        })
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

    pub fn from_hocon(hocon: &Hocon) -> Result<Self> {
        let data = hocon.as_hash().context("window config has to be a map structure")?;
        let position: Option<_> = try {
            (
                data.get("pos")?.as_hash().ok()?.get("x")?.as_i64()? as i32,
                data.get("pos")?.as_hash().ok()?.get("y")?.as_i64()? as i32,
            )
        };
        let size: Option<_> = try {
            (
                data.get("size")?.as_hash().ok()?.get("x")?.as_i64()? as i32,
                data.get("size")?.as_hash().ok()?.get("y")?.as_i64()? as i32,
            )
        };

        let element = WidgetUse::parse_hocon(data.get("widget").context("no widget use given")?.clone())?;

        Ok(EwwWindowDefinition {
            position: position.context("pos.x and pos.y need to be set")?,
            size: size.context("size.x and size.y need to be set")?,
            widget: element,
        })
    }
}

pub fn parse_hocon(s: &str) -> Result<Hocon> {
    let s = s.trim();
    Ok(HoconLoader::new().strict().load_str(s)?.hocon()?)
}
