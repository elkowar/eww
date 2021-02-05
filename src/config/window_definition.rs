use crate::{display_backend, ensure_xml_tag_is};
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
    pub struts: display_backend::StrutDefinition,
    pub focusable: bool,
}

impl EwwWindowDefinition {
    pub fn from_xml_element(xml: &XmlElement) -> Result<Self> {
        ensure_xml_tag_is!(xml, "window");
        let stacking: WindowStacking = xml.parse_optional_attr("stacking")?.unwrap_or_default();

        // TODO maybe rename this to monitor?
        let monitor_name = xml.parse_optional_attr("screen")?;
        let focusable = xml.parse_optional_attr("focusable")?;

        let struts: Option<display_backend::StrutDefinition> = xml
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

fn parse_strut_definition(xml: XmlElement) -> Result<display_backend::StrutDefinition> {
    Ok(display_backend::StrutDefinition {
        side: parse_side(xml.attr("side")?)?,
        dist: xml.attr("distance")?.parse()?,
    })
}

fn parse_side(s: &str) -> Result<display_backend::Side> {
    match s {
        "l" | "left" => Ok(display_backend::Side::Left),
        "r" | "right" => Ok(display_backend::Side::Right),
        "t" | "top" => Ok(display_backend::Side::Top),
        "b" | "bottom" => Ok(display_backend::Side::Bottom),
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

impl Into<display_backend::StackingStrategy> for WindowStacking {
    fn into(self) -> display_backend::StackingStrategy {
        match self {
            WindowStacking::Foreground => display_backend::StackingStrategy::AlwaysOnTop,
            WindowStacking::Background => display_backend::StackingStrategy::AlwaysOnBottom,
        }
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

#[repr(transparent)]
#[derive(Clone, Hash, PartialEq, Eq, AsRef, FromStr, Display, Serialize, Deserialize, Default, From, DebugCustom)]
#[debug(fmt = "WindowName(\".0\")")]
pub struct WindowName(String);

impl std::borrow::Borrow<str> for WindowName {
    fn borrow(&self) -> &str {
        &self.0
    }
}
