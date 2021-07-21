use std::collections::HashMap;

use anyhow::*;
use yuck::config::{
    backend_window_options::BackendWindowOptions,
    widget_definition::WidgetDefinition,
    window_definition::{WindowDefinition, WindowStacking},
    window_geometry::WindowGeometry,
};

use crate::widgets::widget_node;

/// Full window-definition containing the fully expanded widget tree.
/// **Use this** rather than `[RawEwwWindowDefinition]`.
#[derive(Debug, Clone)]
pub struct EwwWindowDefinition {
    pub name: String,

    pub geometry: Option<WindowGeometry>,
    pub stacking: WindowStacking,
    pub monitor_number: Option<i32>,
    pub widget: Box<dyn widget_node::WidgetNode>,
    pub resizable: bool,
    pub backend_options: BackendWindowOptions,
}

impl EwwWindowDefinition {
    pub fn generate(defs: &HashMap<String, WidgetDefinition>, window: WindowDefinition) -> Result<Self> {
        Ok(EwwWindowDefinition {
            name: window.name,
            geometry: window.geometry,
            stacking: window.stacking,
            monitor_number: window.monitor_number,
            resizable: window.resizable,
            widget: widget_node::generate_generic_widget_node(defs, &HashMap::new(), window.widget)?,
            backend_options: window.backend_options,
        })
    }
}
