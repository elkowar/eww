use crate::ensure_xml_tag_is;
use anyhow::*;
use derive_more::*;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct EwwWindowDefinition {
    pub name: WindowName,
    pub geometry: EwwWindowGeometry,
    pub stacking: WindowStacking,
    pub monitor_name: Option<String>,
    pub widget: WidgetUse,
    pub struts: Struts,
    pub focusable: bool,
}

impl EwwWindowDefinition {
    pub fn from_xml_element(xml: &XmlElement) -> Result<Self> {
        ensure_xml_tag_is!(xml, "window");
        let stacking: WindowStacking = xml.parse_optional_attr("stacking")?.unwrap_or_default();

        // TODO maybe rename this to monitor?
        let monitor_name = xml.parse_optional_attr("screen")?;
        let focusable = xml.parse_optional_attr("focusable")?;

        let struts = xml.child("struts").ok().map(Struts::from_xml_element).transpose()?;

        Ok(EwwWindowDefinition {
            name: WindowName(xml.attr("name")?.to_owned()),
            geometry: match xml.child("geometry") {
                Ok(node) => EwwWindowGeometry::from_xml_element(node)?,
                Err(_) => EwwWindowGeometry::default(),
            },
            widget: WidgetUse::from_xml_node(xml.child("widget")?.only_child()?)?,
            stacking,
            monitor_name,
            focusable: focusable.unwrap_or(false),
            struts: struts.unwrap_or_default(),
        })
    }

    /// returns all the variables that are referenced in this window
    pub fn referenced_vars(&self) -> impl Iterator<Item = &VarName> {
        self.widget.referenced_vars()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Struts {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

impl Struts {
    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        ensure_xml_tag_is!(xml, "struts");
        Ok(Struts {
            left: xml.attr("left")?.parse()?,
            right: xml.attr("right")?.parse()?,
            top: xml.attr("top")?.parse()?,
            bottom: xml.attr("bottom")?.parse()?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, derive_more::Display, SmartDefault)]
pub enum WindowStacking {
    #[default]
    Foreground,
    Background,
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

#[repr(transparent)]
#[derive(Clone, Hash, PartialEq, Eq, AsRef, FromStr, Display, Serialize, Deserialize, Default, From, DebugCustom)]
#[debug(fmt = "WindowName(\".0\")")]
pub struct WindowName(String);

impl std::borrow::Borrow<str> for WindowName {
    fn borrow(&self) -> &str {
        &self.0
    }
}
