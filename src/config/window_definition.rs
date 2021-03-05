use crate::{
    ensure_xml_tag_is,
    value::{Coords, NumWithUnit},
    widgets::widget_node,
};
use anyhow::*;
use derive_more::*;
use serde::{Deserialize, Serialize};
use crate::value::Coords;
use smart_default::SmartDefault;
use std::collections::HashMap;

use super::*;

/// Full window-definition containing the fully expanded widget tree.
/// **Use this** rather than `[RawEwwWindowDefinition]`.
#[derive(Debug, Clone)]
pub struct EwwWindowDefinition {
    pub name: WindowName,
    pub geometry: EwwWindowGeometry,
    pub stacking: WindowStacking,
    pub screen_number: Option<i32>,
    pub widget: Box<dyn widget_node::WidgetNode>,
    pub struts: SurfaceDefinition,
    pub focusable: bool,
}

impl EwwWindowDefinition {
    pub fn generate(defs: &HashMap<String, WidgetDefinition>, window: RawEwwWindowDefinition) -> Result<Self> {
        Ok(EwwWindowDefinition {
            name: window.name,
            geometry: window.geometry,
            stacking: window.stacking,
            screen_number: window.screen_number,
            widget: widget_node::generate_generic_widget_node(defs, &HashMap::new(), window.widget)?,
            struts: window.struts,
            focusable: window.focusable,
        })
    }
}

/// Window-definition storing the raw WidgetUse, as received directly from parsing.
#[derive(Debug, Clone, PartialEq)]
pub struct RawEwwWindowDefinition {
    pub name: WindowName,
    pub geometry: EwwWindowGeometry,
    pub stacking: WindowStacking,
    pub screen_number: Option<i32>,
    pub widget: WidgetUse,
    pub struts: SurfaceDefinition,
    pub focusable: bool,
}

impl RawEwwWindowDefinition {
    pub fn from_xml_element(xml: &XmlElement) -> Result<Self> {
        ensure_xml_tag_is!(xml, "window");
        let stacking: WindowStacking = xml.parse_optional_attr("stacking")?.unwrap_or_default();

        // TODO maybe rename this to monitor?
        // Yes, you should
        let focusable = xml.parse_optional_attr("focusable")?;
        let screen_number = xml.parse_optional_attr("screen")?;

        let struts: Option<StrutDefinition> =
            xml.child("reserve").ok().map(StrutDefinition::from_xml_element).transpose().context("Failed to parse <reserve>")?;

        Ok(RawEwwWindowDefinition {
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
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, smart_default::SmartDefault)]
pub enum Side {
    #[default]
    Top,
    Left,
    Right,
    Bottom,
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl std::str::FromStr for Side {
    type Err = anyhow::Error;

    #[cfg(feature = "x11")]
    fn from_str(s: &str) -> Result<Side> {
        match s {
            "l" | "left" => Ok(Side::Left),
            "r" | "right" => Ok(Side::Right),
            "t" | "top" => Ok(Side::Top),
            "b" | "bottom" => Ok(Side::Bottom),
            "c" | "center" => Ok(Side::Center),
            "tl" | "top-left" => Ok(Side::Top_Left),
            "tr" | "top-right" => Ok(Side::Top_Right),
            "bl" | "bottom-left" => Ok(Side::Bottom_Left),
            "br" | "bottom-right" => Ok(Side::Bottom_Right),
            _ => Err(anyhow!("Failed to parse {} as valid side. Must be one of \"left\", \"right\", \"top\", \"bottom\"", s)),
        }
    }

    #[cfg(feature = "wayland")]
    fn from_str(s: &str) -> Result<Side> {
        match s {
            "l" | "left" => Ok(Side::Left),
            "r" | "right" => Ok(Side::Right),
            "t" | "top" => Ok(Side::Top),
            "b" | "bottom" => Ok(Side::Bottom),
            "c" | "center" => Ok(Side::Center),
            "tl" | "top-left" => Ok(Side::TopLeft),
            "tr" | "top-right" => Ok(Side::TopRight),
            "bl" | "bottom-left" => Ok(Side::BottomLeft),
            "br" | "bottom-right" => Ok(Side::BottomRight),
            _ => Err(anyhow!(
                r#"Failed to parse {} as valid side. Must be one of "left", "right", "top", "bottom", "top-right", "top-left", "bottom-left", "bottom-right""#,
                s
            )),
        }
    }
}

// Surface definition if the backend for X11 is enable
#[cfg(feature = "x11")]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub struct SurfaceDefinition {
    pub side: Side,
    pub dist: NumWithUnit,
}

#[cfg(feature = "x11")]
impl StrutDefinition {
    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        Ok(StrutDefinition { side: xml.attr("side")?.parse()?, dist: xml.attr("distance")?.parse()? })
    }
}

// Surface definition if the backend for Wayland is enable
#[cfg(feature = "wayland")]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub struct StrutDefinition {
    pub exclusive: bool,
    pub side: Side,
    pub coords: Coords,
}

#[cfg(feature = "wayland")]
impl StrutDefinition {
    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        Ok(SurfaceDefinition {
            side: xml.attr("side")?.parse()?,
            exclusive: xml.attr("exclusive")?.parse()?,
            coords: Coords {
                x: xml.attr("x")?.parse()?,
                y: xml.attr("y")?.parse()?,
            },
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, derive_more::Display, SmartDefault)]
pub enum WindowStacking {
    #[default]
    Foreground,
    Background,
    Bottom,
    Overlay,
}

impl std::str::FromStr for WindowStacking {
    type Err = anyhow::Error;

    #[cfg(feature = "x11")]
    fn from_str(s: &str) -> Result<Self> {
        let s = s.to_lowercase();
        match s.as_str() {
            "foreground" | "fg" => Ok(WindowStacking::Foreground),
            "background" | "bg" => Ok(WindowStacking::Background),
            "bottom" | "bt" => Ok(WindowStacking::Bottom),
            "overlay" | "ov" => Ok(WindowStacking::Overlay),
            #[cfg(feature = "x11")]
            _ => Err(anyhow!(
                "Couldn't parse '{}' as window stacking, must be either foreground, fg, background or bg",
                s
            )),
            #[cfg(feature = "wayland")]
            _ => Err(anyhow!(
                "Couldn't parse '{}' as window stacking, must be either foreground, fg, background, bg, bottom, bt, overlay or ov",
                s
            )),
        }
    }

    #[cfg(feature = "wayland")]
    fn from_str(s: &str) -> Result<Self> {
        let s = s.to_lowercase();
        match s.as_str() {
            "foreground" | "fg" => Ok(WindowStacking::Foreground),
            "background" | "bg" => Ok(WindowStacking::Background),
            "bottom" | "bt" => Ok(WindowStacking::Bottom),
            "overlay" | "ov" => Ok(WindowStacking::Overlay),
            _ => Err(anyhow!(
                "Couldn't parse '{}' as window stacking, must be either foreground, fg, background, bg, bottom, bt, overlay or \
                 ov",
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
