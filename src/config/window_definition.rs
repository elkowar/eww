use super::{backend_window_options::*, *};
use crate::{ensure_xml_tag_is, enum_parse, value::NumWithUnit, widgets::widget_node};
use derive_more::*;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::{collections::HashMap, str::FromStr};

/// Full window-definition containing the fully expanded widget tree.
/// **Use this** rather than [RawEwwWindowDefinition].
#[derive(Debug, Clone)]
pub struct EwwWindowDefinition {
    pub name: WindowName,

    pub geometry: Option<EwwWindowGeometry>,
    pub stacking: WindowStacking,
    pub screen_number: Option<i32>,
    pub widget: Box<dyn widget_node::WidgetNode>,
    pub resizable: bool,
    pub backend_options: BackendWindowOptions,
}

impl EwwWindowDefinition {
    pub fn generate(defs: &HashMap<String, WidgetDefinition>, window: RawEwwWindowDefinition) -> Result<Self> {
        Ok(EwwWindowDefinition {
            name: window.name,
            geometry: window.geometry,
            stacking: window.stacking,
            screen_number: window.screen_number,
            resizable: window.resizable,
            widget: widget_node::generate_generic_widget_node(defs, &HashMap::new(), window.widget)?,
            backend_options: window.backend_options,
        })
    }
}

/// Window-definition storing the raw WidgetUse, as received directly from parsing.
#[derive(Debug, Clone, PartialEq)]
pub struct RawEwwWindowDefinition {
    pub name: WindowName,
    pub geometry: Option<EwwWindowGeometry>,
    pub stacking: WindowStacking,
    pub widget: WidgetUse,
    pub resizable: bool,
    pub backend_options: BackendWindowOptions,
    pub screen_number: Option<i32>,
}

impl RawEwwWindowDefinition {
    pub fn from_xml_element(xml: &XmlElement) -> Result<Self> {
        ensure_xml_tag_is!(xml, "window");
        let geometry = match xml.child("geometry") {
            Ok(node) => Some(EwwWindowGeometry::from_xml_element(node)?),
            Err(_) => None,
        };

        Ok(RawEwwWindowDefinition {
            name: WindowName(xml.attr("name")?),
            geometry,
            widget: WidgetUse::from_xml_node(xml.child("widget")?.only_child()?)?,
            stacking: xml.parse_optional_attr("stacking")?.unwrap_or_default(),
            // TODO maybe rename this to monitor?
            screen_number: xml.parse_optional_attr("screen")?,
            resizable: xml.parse_optional_attr("resizable")?.unwrap_or(true),
            backend_options: BackendWindowOptions::from_xml_element(xml)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, SmartDefault)]
pub enum EwwWindowType {
    #[default]
    Dock,
    Dialog,
    Toolbar,
    Normal,
    Utility,
}
impl FromStr for EwwWindowType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        enum_parse! { "window type", s,
            "dock" => Self::Dock,
            "toolbar" => Self::Toolbar,
            "dialog" => Self::Dialog,
            "normal" => Self::Normal,
            "utility" => Self::Utility,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, smart_default::SmartDefault)]
pub enum Side {
    #[default]
    Top,
    Left,
    Right,
    Bottom,
}

impl std::str::FromStr for Side {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Side> {
        enum_parse! { "side", s,
            "l" | "left" => Side::Left,
            "r" | "right" => Side::Right,
            "t" | "top" => Side::Top,
            "b" | "bottom" => Side::Bottom,
        }
    }
}

// Surface definition if the backend for X11 is enable
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub struct StrutDefinition {
    pub side: Side,
    pub dist: NumWithUnit,
}

impl StrutDefinition {
    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        Ok(StrutDefinition { side: xml.attr("side")?.parse()?, dist: xml.attr("distance")?.parse()? })
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

    #[cfg(not(feature = "wayland"))]
    fn from_str(s: &str) -> Result<Self> {
        enum_parse! { "WindowStacking", s,
            "foreground" | "fg" | "f" => WindowStacking::Foreground,
            "background" | "bg" | "b" => WindowStacking::Background,
        }
    }

    #[cfg(feature = "wayland")]
    fn from_str(s: &str) -> Result<Self> {
        enum_parse! { "WindowStacking", s,
            "foreground" | "fg" => WindowStacking::Foreground,
            "background" | "bg" => WindowStacking::Background,
            "bottom" | "bt" => WindowStacking::Bottom,
            "overlay" | "ov" => WindowStacking::Overlay,
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
