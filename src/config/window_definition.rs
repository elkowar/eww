use crate::{ensure_xml_tag_is, value::Coords};
use anyhow::*;
use derive_more::*;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

use super::*;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct EwwWindowDefinition {
    pub position: Coords,
    pub size: Coords,
    pub stacking: WindowStacking,
    pub screen_number: Option<i32>,
    pub widget: WidgetUse,
    pub struts: Struts,
}

impl EwwWindowDefinition {
    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        ensure_xml_tag_is!(xml, "window");

        let size_node = xml.child("size")?;
        let size = Coords::from_strs(size_node.attr("x")?, size_node.attr("y")?)?;
        let pos_node = xml.child("pos")?;
        let position = Coords::from_strs(pos_node.attr("x")?, pos_node.attr("y")?)?;

        let stacking = xml.attr("stacking").ok().map(|x| x.parse()).transpose()?.unwrap_or_default();
        let screen_number = xml.attr("screen").ok().map(|x| x.parse()).transpose()?;
        let struts = xml.child("struts").ok().map(Struts::from_xml_element).transpose()?;

        let widget = WidgetUse::from_xml_node(xml.child("widget")?.only_child()?)?;
        Ok(EwwWindowDefinition {
            position,
            size,
            widget,
            stacking,
            screen_number,
            struts: struts.unwrap_or_default(),
        })
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
#[derive(Clone, Hash, PartialEq, Eq, AsRef, FromStr, Display, Serialize, Deserialize, Default, From)]
pub struct WindowName(String);

impl std::borrow::Borrow<str> for WindowName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for WindowName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WindowName(\"{}\")", self.0)
    }
}
