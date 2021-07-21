use super::*;
use crate::{dynval::NumWithUnit, ensure_xml_tag_is, widgets::widget_node};
use derive_more::*;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::{collections::HashMap, str::FromStr};
use yuck::config::{
    backend_window_options::StrutDefinition,
    window_definition::{WindowDefinition, WindowStacking},
    window_geometry::WindowGeometry,
};

#[derive(Debug, Clone, PartialEq)]
pub enum EwwWindowType {
    Dock,
    Dialog,
    Toolbar,
    Normal,
}
impl FromStr for EwwWindowType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "dock" => Ok(Self::Dock),
            "toolbar" => Ok(Self::Toolbar),
            "dialog" => Ok(Self::Dialog),
            "normal" => Ok(Self::Normal),
            x => Err(anyhow!("Unknown windowtype provided '{}'. Possible values are: dock, toolbar, dialog, normal", x)),
        }
    }
}

impl Default for EwwWindowType {
    fn default() -> Self {
        Self::Normal
    }
}

/// Full window-definition containing the fully expanded widget tree.
/// **Use this** rather than `[RawEwwWindowDefinition]`.
#[derive(Debug, Clone)]
pub struct EwwWindowDefinition {
    pub name: String,

    pub geometry: WindowGeometry,
    pub stacking: WindowStacking,
    pub monitor_number: Option<i32>,
    pub widget: Box<dyn widget_node::WidgetNode>,
    pub focusable: bool,

    #[cfg(feature = "x11")]
    pub window_type: EwwWindowType,

    #[cfg(feature = "x11")]
    pub struts: StrutDefinition,

    #[cfg(feature = "wayland")]
    pub exclusive: bool,
}

impl EwwWindowDefinition {
    pub fn generate(defs: &HashMap<String, WidgetDefinition>, window: WindowDefinition) -> Result<Self> {
        Ok(EwwWindowDefinition {
            name: window.name,
            geometry: window.geometry,
            stacking: window.stacking,
            monitor_number: window.screen_number,
            widget: widget_node::generate_generic_widget_node(defs, &HashMap::new(), window.widget)?,
            focusable: window.focusable,
            #[cfg(feature = "x11")]
            window_type: window.window_type,
            #[cfg(feature = "x11")]
            struts: window.struts,
            #[cfg(feature = "wayland")]
            exclusive: window.exclusive,
        })
    }
}
