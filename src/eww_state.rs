use crate::{
    config::WindowName,
    util,
    value::{AttrName, AttrValueElement, VarName},
};
use anyhow::*;
use std::{collections::HashMap, process::Command, sync::Arc};

use crate::value::{AttrValue, PrimitiveValue};

/// Handler that get's executed to apply the necessary parts of the eww state to
/// a gtk widget. These are created and initialized in EwwState::resolve.
pub struct StateChangeHandler {
    func: Box<dyn Fn(HashMap<AttrName, PrimitiveValue>) -> Result<()> + 'static>,
    unresolved_values: Vec<(AttrName, AttrValue)>,
}

impl StateChangeHandler {
    fn used_variables(&self) -> impl Iterator<Item = &VarName> {
        self.unresolved_values.iter().flat_map(|(_, value)| value.var_refs())
    }

    /// Run the StateChangeHandler.
    /// [`state`] should be the global [EwwState::state].
    fn run_with_state(&self, state: &HashMap<VarName, PrimitiveValue>) {
        let resolved_attrs = self
            .unresolved_values
            .iter()
            .cloned()
            .map(|(attr_name, value)| Ok((attr_name, value.resolve_fully(state)?)))
            .collect::<Result<_>>();

        match resolved_attrs {
            Ok(resolved_attrs) => {
                let result: Result<_> = (self.func)(resolved_attrs);
                util::print_result_err("while updating UI based after state change", &result);
            }
            Err(err) => {
                eprintln!("Error whiel resolving attributes {:?}", err);
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
            self.state_change_handlers
                .entry(var_name.clone())
                .or_insert_with(Vec::new)
                .push(handler.clone());
        }
    }
}

/// Stores the actual state of eww, including the variable state and the
/// window-specific state-change handlers.
#[derive(Default)]
pub struct EwwState {
    windows: HashMap<WindowName, EwwWindowState>,
    variables_state: HashMap<VarName, PrimitiveValue>,
}

impl std::fmt::Debug for EwwState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EwwState {{ state: {:?} }}", self.variables_state)
    }
}

impl EwwState {
    pub fn from_default_vars(defaults: HashMap<VarName, PrimitiveValue>) -> Self {
        EwwState {
            variables_state: defaults,
            ..EwwState::default()
        }
    }

    pub fn get_variables(&self) -> &HashMap<VarName, PrimitiveValue> {
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
    pub fn update_variable(&mut self, key: VarName, value: PrimitiveValue) -> Result<()> {
        self.variables_state.insert(key.clone(), value);

        let handlers = self
            .windows
            .values()
            .filter_map(|window_state| window_state.state_change_handlers.get(&key))
            .flatten();

        for handler in handlers {
            handler.run_with_state(&self.variables_state)
        }
        Ok(())
    }

    /// resolves a value if possible, using the current eww_state
    /// Expects there to be at max one level of nesting var_refs from local-env.
    /// This means that no elements in the local_env may be var-refs into the
    /// local_env again, but only into the global state.
    pub fn resolve_once<'a>(
        &'a self,
        local_env: &'a HashMap<VarName, AttrValue>,
        value: &'a AttrValue,
    ) -> Result<PrimitiveValue> {
        value
            .iter()
            .map(|element| match element {
                AttrValueElement::Primitive(primitive) => Ok(primitive.clone()),
                AttrValueElement::VarRef(var_name) => self
                    .variables_state
                    .get(var_name)
                    .cloned()
                    .or_else(|| local_env.get(var_name).and_then(|x| self.resolve_once(local_env, x).ok()))
                    .with_context(|| format!("Unknown variable '{}' referenced", var_name)),
            })
            .collect()
    }

    /// Resolve takes a function that applies a set of fully resolved attribute
    /// values to it's gtk widget. Expects there to be at max one level of
    /// nesting var_refs from local-env. This means that no elements in the
    /// local_env may be var-refs into the local_env again, but only into the
    /// global state.
    pub fn resolve<F: Fn(HashMap<AttrName, PrimitiveValue>) -> Result<()> + 'static + Clone>(
        &mut self,
        window_name: &WindowName,
        local_env: &HashMap<VarName, AttrValue>,
        attributes: HashMap<AttrName, AttrValue>,
        set_value: F,
    ) {
        let handler = StateChangeHandler {
            func: Box::new(set_value),
            unresolved_values: attributes
                .into_iter()
                .map(|(attr_name, attr_value)| (attr_name, attr_value.resolve_one_level(local_env)))
                .collect(),
        };

        handler.run_with_state(&self.variables_state);

        // only store the handler if at least one variable is being used
        if handler.used_variables().next().is_some() {
            let window_state = self
                .windows
                .entry(window_name.clone())
                .or_insert_with(EwwWindowState::default);
            window_state.put_handler(handler);
        }
    }
}

/// Run a command and get the output
pub fn run_command(cmd: &str) -> Result<PrimitiveValue> {
    let output = String::from_utf8(Command::new("/bin/sh").arg("-c").arg(cmd).output()?.stdout)?;
    let output = output.trim_matches('\n');
    Ok(PrimitiveValue::from(output))
}
