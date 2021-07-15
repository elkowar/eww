use super::{backend_window_options::*, *};
use crate::{ensure_xml_tag_is, value::NumWithUnit, widgets::widget_node};
use derive_more::*;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::{collections::HashMap, str::FromStr};

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
        match s.to_lowercase().as_str() {
            "dock" => Ok(Self::Dock),
            "toolbar" => Ok(Self::Toolbar),
            "dialog" => Ok(Self::Dialog),
            "normal" => Ok(Self::Normal),
            "utility" => Ok(Self::Utility),
            x => Err(anyhow!("Unknown windowtype provided '{}'. Possible values are: dock, toolbar, dialog, normal, utility", x)),
        }
    }
}

/// Full window-definition containing the fully expanded widget tree.
/// **Use this** rather than `[RawEwwWindowDefinition]`.
#[derive(Debug, Clone)]
pub struct EwwWindowDefinition {
    pub name: WindowName,

    pub geometry: Option<EwwWindowGeometry>,
    pub stacking: WindowStacking,
    pub screen_number: Option<i32>,
    pub widget: Box<dyn widget_node::WidgetNode>,
    pub focusable: bool,
    pub backend_options: BackendWindowOptions,
}

impl EwwWindowDefinition {
    pub fn generate(defs: &HashMap<String, WidgetDefinition>, window: RawEwwWindowDefinition) -> Result<Self> {
        Ok(EwwWindowDefinition {
            name: window.name,
            geometry: window.geometry,
            stacking: window.stacking,
            screen_number: window.screen_number,
            widget: widget_node::generate_generic_widget_node(defs, &HashMap::new(), window.widget)?,
            focusable: window.focusable,
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
    pub focusable: bool,
    pub backend_options: BackendWindowOptions,
    // TODO maybe rename this to monitor?
    pub screen_number: Option<i32>,
}

impl RawEwwWindowDefinition {
    pub fn from_xml_element(xml: &XmlElement) -> Result<Self> {
        ensure_xml_tag_is!(xml, "window");

        Ok(RawEwwWindowDefinition {
            name: WindowName(xml.attr("name")?),
            geometry: match xml.child("geometry") {
                Ok(node) => Some(EwwWindowGeometry::from_xml_element(node)?),
                Err(_) => None,
            },
            widget: WidgetUse::from_xml_node(xml.child("widget")?.only_child()?)?,
            stacking: xml.parse_optional_attr("stacking")?.unwrap_or_default(),
            screen_number: xml.parse_optional_attr("screen")?,
            focusable: xml.parse_optional_attr("focusable")?.unwrap_or(false),
            backend_options: BackendWindowOptions::from_xml_element(xml)?,
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
}

impl std::str::FromStr for Side {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Side> {
        match s {
            "l" | "left" => Ok(Side::Left),
            "r" | "right" => Ok(Side::Right),
            "t" | "top" => Ok(Side::Top),
            "b" | "bottom" => Ok(Side::Bottom),
            _ => Err(anyhow!("Failed to parse {} as valid side. Must be one of \"left\", \"right\", \"top\", \"bottom\"", s)),
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
        let s = s.to_lowercase();
        match s.as_str() {
            "foreground" | "fg" | "f" => Ok(WindowStacking::Foreground),
            "background" | "bg" | "b" => Ok(WindowStacking::Background),
            _ => Err(anyhow!("Couldn't parse '{}' as window stacking, must be either foreground, fg, background or bg", s)),
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
