use anyhow::Result;
use eww_shared_util::{AttrName, VarName};
use simplexpr::{dynval::DynVal, SimplExpr};
use std::collections::HashMap;
use yuck::config::{
    backend_window_options::BackendWindowOptions,
    monitor::MonitorIdentifier,
    window_definition::{WindowDefinition, WindowStacking},
    window_geometry::WindowGeometry,
};

use crate::window_arguments::WindowArguments;

/// This stores all the information required to create a window and is created
/// via combining information from the WindowDefinition and the WindowInitiator
#[derive(Debug, Clone)]
pub struct WindowInitiator {
    pub backend_options: BackendWindowOptions,
    pub geometry: Option<WindowGeometry>,
    pub id: String,
    pub local_variables: HashMap<VarName, DynVal>,
    pub monitor: Option<MonitorIdentifier>,
    pub name: String,
    pub resizable: bool,
    pub stacking: WindowStacking,
}

impl WindowInitiator {
    pub fn new(window_def: &WindowDefinition, args: &WindowArguments) -> Result<Self> {
        let vars = args.get_local_window_variables(window_def)?;

        let backend_options = window_def.backend_options.eval(&vars)?;
        let geometry = match &window_def.geometry {
            Some(geo) => Some(geo.eval(&vars)?.override_if_given(args.anchor, args.pos, args.size)),
            None => None,
        };
        let monitor = if args.monitor.is_none() { window_def.eval_monitor(&vars)? } else { args.monitor.clone() };
        let resizable = window_def.eval_resizable(&vars)?;
        let stacking = window_def.eval_stacking(&vars)?;

        Ok(WindowInitiator {
            backend_options,
            geometry,
            id: args.id.clone(),
            local_variables: vars,
            monitor,
            name: window_def.name.clone(),
            resizable,
            stacking,
        })
    }

    pub fn get_scoped_vars(&self) -> HashMap<AttrName, SimplExpr> {
        self.local_variables.iter().map(|(k, v)| (AttrName::from(k.clone()), SimplExpr::Literal(v.clone()))).collect()
    }
}
