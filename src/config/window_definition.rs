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
    pub screen_number: Option<i32>,
    pub widget: WidgetUse,
    pub struts: Struts,
    pub focusable: bool,
    pub sticky: bool,
}

impl EwwWindowDefinition {
    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        ensure_xml_tag_is!(xml, "window");
        let stacking: WindowStacking = xml.parse_optional_attr("stacking")?.unwrap_or_default();
        let screen_number = xml.parse_optional_attr("screen")?;
        let focusable = xml.parse_optional_attr("focusable")?;
        let sticky = xml.parse_optional_attr("sticky")?;

        let struts = xml.child("struts").ok().map(Struts::from_xml_element).transpose()?;

        Ok(EwwWindowDefinition {
            name: WindowName(xml.attr("name")?.to_owned()),
            geometry: match xml.child("geometry") {
                Ok(node) => EwwWindowGeometry::from_xml_element(node)?,
                Err(_) => EwwWindowGeometry::default(),
            },
            widget: WidgetUse::from_xml_node(xml.child("widget")?.only_child()?)?,
            stacking,
            screen_number,
            focusable: focusable.unwrap_or(false),
            sticky: sticky.unwrap_or(true),
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
    pub left: u32,
    pub right: u32,
    pub top: u32,
    pub bottom: u32,
    pub left_start_y: u32,
    pub left_end_y: u32,
    pub right_start_y: u32,
    pub right_end_y: u32,
    pub top_start_x: u32,
    pub top_end_x: u32,
    pub bottom_start_x: u32,
    pub bottom_end_x: u32,
}

impl Struts {
    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        ensure_xml_tag_is!(xml, "struts");
        Ok(Struts {
            left: xml.parse_optional_attr("left")?.unwrap_or(0),
            right: xml.parse_optional_attr("right")?.unwrap_or(0),
            top: xml.parse_optional_attr("top")?.unwrap_or(0),
            bottom: xml.parse_optional_attr("bottom")?.unwrap_or(0),
            left_start_y: xml.parse_optional_attr("left_start_y")?.unwrap_or(0),
            left_end_y: xml.parse_optional_attr("left_end_y")?.unwrap_or(0),
            right_start_y: xml.parse_optional_attr("right_start_y")?.unwrap_or(0),
            right_end_y: xml.parse_optional_attr("right_end_y")?.unwrap_or(0),
            top_start_x: xml.parse_optional_attr("top_start_x")?.unwrap_or(0),
            top_end_x: xml.parse_optional_attr("top_end_x")?.unwrap_or(0),
            bottom_start_x: xml.parse_optional_attr("bottom_start_x")?.unwrap_or(0),
            bottom_end_x: xml.parse_optional_attr("bottom_end_x")?.unwrap_or(0),
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
