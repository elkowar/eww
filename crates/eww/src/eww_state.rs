use anyhow::*;
use eww_shared_util::{AttrName, VarName};
use std::{collections::HashMap, sync::Arc};

use simplexpr::{dynval::DynVal, SimplExpr};

use crate::error_handling_ctx;

/// Handler that gets executed to apply the necessary parts of the eww state to
/// a gtk widget. These are created and initialized in EwwState::resolve.
pub struct StateChangeHandler {
    func: Box<dyn Fn(HashMap<AttrName, DynVal>) -> Result<()> + 'static>,
    unresolved_values: HashMap<AttrName, SimplExpr>,
}

impl StateChangeHandler {
    fn used_variables(&self) -> impl Iterator<Item = &VarName> {
        self.unresolved_values.iter().flat_map(|(_, value)| value.var_refs_with_span()).map(|(_, value)| value)
    }

    /// Run the StateChangeHandler.
    /// [`state`] should be the global [EwwState::state].
    fn run_with_state(&self, state: &HashMap<VarName, DynVal>) {
        let resolved_attrs = self
            .unresolved_values
            .clone()
            .into_iter()
            .map(|(attr_name, value)| Ok((attr_name, value.eval(state)?)))
            .collect::<Result<_>>();

        match resolved_attrs {
            Ok(resolved_attrs) => {
                if let Err(err) = (self.func)(resolved_attrs).context("Error while updating UI after state change") {
                    error_handling_ctx::print_error(err);
                }
            }
            Err(err) => {
                error_handling_ctx::print_error(err);
            }
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
    windows: HashMap<String, EwwWindowState>,
    variables_state: HashMap<VarName, DynVal>,
}

impl std::fmt::Debug for EwwState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EwwState {{ state: {:?} }}", self.variables_state)
    }
}

impl EwwState {
    pub fn from_default_vars(defaults: HashMap<VarName, DynVal>) -> Self {
        EwwState { variables_state: defaults, ..EwwState::default() }
    }

    pub fn get_variables(&self) -> &HashMap<VarName, DynVal> {
        &self.variables_state
    }

    /// remove all state stored specific to one window
    pub fn clear_window_state(&mut self, window_name: &str) {
        self.windows.remove(window_name);
    }

    /// remove all state that is specific to any window
    pub fn clear_all_window_states(&mut self) {
        self.windows.clear();
    }

    /// Update the value of a variable, running all registered
    /// [StateChangeHandler]s.
    pub fn update_variable(&mut self, key: VarName, value: DynVal) {
        self.variables_state.insert(key.clone(), value);

        // run all of the handlers
        self.windows
            .values()
            .filter_map(|window_state| window_state.state_change_handlers.get(&key))
            .flatten()
            .for_each(|handler| handler.run_with_state(&self.variables_state));
    }

    /// Look up a single variable in the eww state, returning an `Err` when the value is not found.
    pub fn lookup(&self, var_name: &VarName) -> Result<&DynVal> {
        self.variables_state.get(var_name).with_context(|| format!("Unknown variable '{}' referenced", var_name))
    }

    /// resolves a value if possible, using the current eww_state.
    pub fn resolve_once<'a>(&'a self, value: &'a SimplExpr) -> Result<DynVal> {
        Ok(value.clone().eval(&self.variables_state)?)
    }

    /// Resolve takes a function that applies a set of fully resolved attribute
    /// values to it's gtk widget.
    pub fn resolve<F: Fn(HashMap<AttrName, DynVal>) -> Result<()> + 'static + Clone>(
        &mut self,
        window_name: &str,
        required_attributes: HashMap<AttrName, SimplExpr>,
        set_value: F,
    ) {
        let handler = StateChangeHandler { func: Box::new(set_value), unresolved_values: required_attributes };

        handler.run_with_state(&self.variables_state);

        // only store the handler if at least one variable is being used
        if handler.used_variables().next().is_some() {
            self.windows.entry(window_name.to_string()).or_insert_with(EwwWindowState::default).put_handler(handler);
        }
    }

    pub fn referenced_vars(&self) -> impl Iterator<Item = &VarName> {
        self.windows.values().flat_map(|w| w.state_change_handlers.keys())
    }

    pub fn vars_referenced_in(&self, window_name: &str) -> std::collections::HashSet<&VarName> {
        self.windows.get(window_name).map(|window| window.state_change_handlers.keys().collect()).unwrap_or_default()
    }
}
