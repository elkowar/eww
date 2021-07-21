use crate::{
    config::window_definition::WindowName,
    value::{AttrName, AttrValElement, VarName},
};
use anyhow::*;
use std::{collections::HashMap, sync::Arc};

use crate::value::{AttrVal, PrimVal};

/// Handler that gets executed to apply the necessary parts of the eww state to
/// a gtk widget. These are created and initialized in EwwState::resolve.
pub struct StateChangeHandler {
    func: Box<dyn Fn(HashMap<AttrName, PrimVal>) -> Result<()> + 'static>,
    unresolved_values: HashMap<AttrName, AttrVal>,
}

impl StateChangeHandler {
    fn used_variables(&self) -> impl Iterator<Item = &VarName> {
        self.unresolved_values.iter().flat_map(|(_, value)| value.var_refs())
    }

    /// Run the StateChangeHandler.
    /// [`state`] should be the global [EwwState::state].
    fn run_with_state(&self, state: &HashMap<VarName, PrimVal>) {
        let resolved_attrs = self
            .unresolved_values
            .clone()
            .into_iter()
            .map(|(attr_name, value)| Ok((attr_name, value.resolve_fully(state)?)))
            .collect::<Result<_>>();

        match resolved_attrs {
            Ok(resolved_attrs) => {
                crate::print_result_err!("while updating UI based after state change", &(self.func)(resolved_attrs))
            }
            Err(err) => log::error!("Error while resolving attributes: {:?}", err),
        }
    }
}

/// Collection of [StateChangeHandler]s
/// State specific to one window.
/// stores the state_change handlers that are used for that window.
#[derive(Default)]
pub struct EwwWindowState {
    state_change_handlers: HashMap<VarName, Vec<Arc<StateChangeHandler>>>,
}

impl EwwWindowState {
    /// register a new [`StateChangeHandler`]
    fn put_handler(&mut self, handler: StateChangeHandler) {
        let handler = Arc::new(handler);
        for var_name in handler.used_variables() {
            self.state_change_handlers.entry(var_name.clone()).or_insert_with(Vec::new).push(handler.clone());
        }
    }
}

/// Stores the actual state of eww, including the variable state and the
/// window-specific state-change handlers.
#[derive(Default)]
pub struct EwwState {
    windows: HashMap<WindowName, EwwWindowState>,
    variables_state: HashMap<VarName, PrimVal>,
}

impl std::fmt::Debug for EwwState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EwwState {{ state: {:?} }}", self.variables_state)
    }
}

impl EwwState {
    pub fn from_default_vars(defaults: HashMap<VarName, PrimVal>) -> Self {
        EwwState { variables_state: defaults, ..EwwState::default() }
    }

    pub fn get_variables(&self) -> &HashMap<VarName, PrimVal> {
        &self.variables_state
    }

    /// remove all state stored specific to one window
    pub fn clear_window_state(&mut self, window_name: &WindowName) {
        self.windows.remove(window_name);
    }

    /// remove all state that is specific to any window
    pub fn clear_all_window_states(&mut self) {
        self.windows.clear();
    }

    /// Update the value of a variable, running all registered
    /// [StateChangeHandler]s.
    pub fn update_variable(&mut self, key: VarName, value: PrimVal) {
        self.variables_state.insert(key.clone(), value);

        // run all of the handlers
        self.windows
            .values()
            .filter_map(|window_state| window_state.state_change_handlers.get(&key))
            .flatten()
            .for_each(|handler| handler.run_with_state(&self.variables_state));
    }

    /// Look up a single variable in the eww state, returning an `Err` when the value is not found.
    pub fn lookup(&self, var_name: &VarName) -> Result<&PrimVal> {
        self.variables_state.get(var_name).with_context(|| format!("Unknown variable '{}' referenced", var_name))
    }

    /// resolves a value if possible, using the current eww_state.
    pub fn resolve_once<'a>(&'a self, value: &'a AttrVal) -> Result<PrimVal> {
        value
            .iter()
            .map(|element| match element {
                AttrValElement::Primitive(primitive) => Ok(primitive.clone()),
                AttrValElement::Expr(expr) => expr.clone().eval(&self.variables_state),
            })
            .collect()
    }

    /// Resolve takes a function that applies a set of fully resolved attribute
    /// values to it's gtk widget.
    pub fn resolve<F: Fn(HashMap<AttrName, PrimVal>) -> Result<()> + 'static + Clone>(
        &mut self,
        window_name: &WindowName,
        required_attributes: HashMap<AttrName, AttrVal>,
        set_value: F,
    ) {
        let handler = StateChangeHandler { func: Box::new(set_value), unresolved_values: required_attributes };

        handler.run_with_state(&self.variables_state);

        // only store the handler if at least one variable is being used
        if handler.used_variables().next().is_some() {
            self.windows.entry(window_name.clone()).or_insert_with(EwwWindowState::default).put_handler(handler);
        }
    }

    pub fn referenced_vars(&self) -> impl Iterator<Item = &VarName> {
        self.windows.values().flat_map(|w| w.state_change_handlers.keys())
    }

    pub fn vars_referenced_in(&self, window_name: &WindowName) -> std::collections::HashSet<&VarName> {
        self.windows.get(window_name).map(|window| window.state_change_handlers.keys().collect()).unwrap_or_default()
    }
}
