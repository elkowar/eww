use crate::{ensure_xml_tag_is, value::NumWithUnit};
use anyhow::*;
use derive_more::*;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

use super::*;

#[derive(Debug, Clone, Copy, Eq, PartialEq, smart_default::SmartDefault)]
pub enum Side {
    #[default]
    Top,
    Left,
    Right,
    Bottom,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub struct StrutDefinition {
    pub side: Side,
    pub dist: NumWithUnit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EwwWindowDefinition {
    pub name: WindowName,
    pub geometry: EwwWindowGeometry,
    pub stacking: WindowStacking,
    pub screen_number: Option<i32>,
    pub widget: WidgetUse,
    pub struts: StrutDefinition,
    pub focusable: bool,
}

impl EwwWindowDefinition {
    pub fn from_xml_element(xml: &XmlElement) -> Result<Self> {
        ensure_xml_tag_is!(xml, "window");
        let stacking: WindowStacking = xml.parse_optional_attr("stacking")?.unwrap_or_default();

        // TODO maybe rename this to monitor?
        let focusable = xml.parse_optional_attr("focusable")?;
        let screen_number = xml.parse_optional_attr("screen")?;

        let struts: Option<StrutDefinition> = xml
            .child("reserve")
            .ok()
            .map(parse_strut_definition)
            .transpose()
            .context("Failed to parse <reserve>")?;

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
            struts: struts.unwrap_or_default(),
        })
    }

    /// returns all the variables that are referenced in this window
    pub fn referenced_vars(&self) -> impl Iterator<Item = &VarName> {
        self.widget.referenced_vars()
    }
}

fn parse_strut_definition(xml: XmlElement) -> Result<StrutDefinition> {
    Ok(StrutDefinition {
        side: parse_side(xml.attr("side")?)?,
        dist: xml.attr("distance")?.parse()?,
    })
}

fn parse_side(s: &str) -> Result<Side> {
    match s {
        "l" | "left" => Ok(Side::Left),
        "r" | "right" => Ok(Side::Right),
        "t" | "top" => Ok(Side::Top),
        "b" | "bottom" => Ok(Side::Bottom),
        _ => Err(anyhow!(
            "Failed to parse {} as valid side. Must be one of \"left\", \"right\", \"top\", \"bottom\"",
            s
        )),
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
