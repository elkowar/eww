use crate::{
    config::WindowName,
    value::{self, AttrName, VarName},
};
use anyhow::*;
use itertools::Itertools;
use std::{collections::HashMap, process::Command, sync::Arc};

use crate::value::{AttrValue, PrimitiveValue};

/// Handler that get's executed to apply the necessary parts of the eww state to
/// a gtk widget. These are created and initialized in EwwState::resolve.
pub struct StateChangeHandler {
    func: Box<dyn Fn(HashMap<AttrName, PrimitiveValue>) -> Result<()> + 'static>,
    constant_values: HashMap<AttrName, PrimitiveValue>,
    unresolved_attrs: HashMap<AttrName, VarName>,
    string_with_varrefs_resolvers: HashMap<AttrName, Box<dyn Fn(&HashMap<VarName, PrimitiveValue>) -> PrimitiveValue>>,
}

impl StateChangeHandler {
    /// Run the StateChangeHandler.
    /// [`state`] should be the global [EwwState::state].
    fn run_with_state(&self, state: &HashMap<VarName, PrimitiveValue>) -> Result<()> {
        let mut all_resolved_attrs = self.constant_values.clone();
        for (attr_name, var_ref) in self.unresolved_attrs.iter() {
            let resolved = state
                .get(var_ref)
                // TODO provide context here, including line numbers
                .with_context(|| format!("Unknown variable '{}' was referenced", var_ref))?;
            all_resolved_attrs.insert(attr_name.to_owned(), resolved.clone());
        }
        for (attr_name, resolver) in self.string_with_varrefs_resolvers.iter() {
            all_resolved_attrs.insert(attr_name.to_owned(), resolver(state));
        }

        let result: Result<_> = (self.func)(all_resolved_attrs);
        if let Err(err) = result {
            eprintln!("WARN: Error while resolving attributes: {}", err);
        }

        Ok(())
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
    /// register a new [StateChangeHandler]
    fn put_handler(&mut self, handler: StateChangeHandler) {
        let handler = Arc::new(handler);
        for var_name in handler.unresolved_attrs.values() {
            let entry: &mut Vec<Arc<StateChangeHandler>> =
                self.state_change_handlers.entry(var_name.clone()).or_insert_with(Vec::new);
            entry.push(handler.clone());
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
            handler
                .run_with_state(&self.variables_state)
                .with_context(|| format!("When updating value of {}", &key))?;
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
        match value {
            AttrValue::Concrete(primitive) => Ok(primitive.clone()),
            AttrValue::VarRef(var_name) => match local_env.get(var_name) {
                // look up if variables are found in the local env, and resolve as far as possible
                Some(AttrValue::Concrete(primitive)) => Ok(primitive.clone()),
                Some(AttrValue::VarRef(var_name)) => self
                    .variables_state
                    .get(var_name)
                    .cloned()
                    .ok_or_else(|| anyhow!("Unknown variable '{}' referenced", var_name)),
                Some(AttrValue::StringWithVarRefs(content)) => content
                    .iter()
                    .map(|x| x.clone().to_attr_value())
                    .map(|value| self.resolve_once(local_env, &value))
                    .fold_results(String::new(), |acc, cur| format!("{}{}", acc, cur))
                    .map(PrimitiveValue::from_string),
                None => self
                    .variables_state
                    .get(var_name)
                    .cloned()
                    .ok_or_else(|| anyhow!("Unknown variable '{}' referenced", var_name)),
            },
            AttrValue::StringWithVarRefs(content) => content
                .iter()
                .map(|x| x.clone().to_attr_value())
                .map(|value| self.resolve_once(local_env, &value))
                .fold_results(String::new(), |acc, cur| format!("{}{}", acc, cur))
                .map(PrimitiveValue::from_string),
        }
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
        mut needed_attributes: HashMap<AttrName, AttrValue>,
        set_value: F,
    ) {
        // Resolve first collects all variable references and creates a set of
        // unresolved attribute -> VarName pairs. additionally, all constant values are
        // looked up and collected, including the values from the local environment
        // These are then used to generate a StateChangeHandler, which is then executed
        // and registered in the windows state.

        let result: Result<_> = try {
            let window_state = self
                .windows
                .entry(window_name.clone())
                .or_insert_with(EwwWindowState::default);

            let mut string_with_varrefs_resolvers: HashMap<_, Box<dyn Fn(&HashMap<VarName, PrimitiveValue>) -> PrimitiveValue>> =
                HashMap::new();

            let mut resolved_attrs = HashMap::new();
            let mut unresolved_attrs: HashMap<AttrName, VarName> = HashMap::new();
            needed_attributes
                .drain()
                .for_each(|(attr_name, attr_value)| match attr_value {
                    // directly resolve primitive values
                    AttrValue::Concrete(primitive) => {
                        resolved_attrs.insert(attr_name, primitive);
                    }
                    AttrValue::StringWithVarRefs(content) => {
                        let content = content.resolve_one_level(local_env);
                        let resolver = generate_string_with_var_refs_resolver(content);
                        string_with_varrefs_resolvers.insert(attr_name, Box::new(resolver));
                    }

                    AttrValue::VarRef(var_name) => match local_env.get(&var_name) {
                        Some(AttrValue::StringWithVarRefs(content)) => {
                            let content = content.clone().resolve_one_level(local_env);
                            let resolver = generate_string_with_var_refs_resolver(content);
                            string_with_varrefs_resolvers.insert(attr_name, Box::new(resolver));
                        }

                        // look up if variables are found in the local env, and resolve as far as possible
                        Some(AttrValue::Concrete(concrete_from_local)) => {
                            resolved_attrs.insert(attr_name, concrete_from_local.clone());
                        }
                        Some(AttrValue::VarRef(var_ref_from_local)) => {
                            unresolved_attrs.insert(attr_name, var_ref_from_local.clone());
                        }
                        None => {
                            // if it's not in the local env, it must reference the global state,
                            // and should thus directly be inserted into the unresolved attrs.
                            unresolved_attrs.insert(attr_name, var_name);
                        }
                    },
                });

            if unresolved_attrs.is_empty() && string_with_varrefs_resolvers.is_empty() {
                // if there are no unresolved variables, we can set the value directly
                set_value(resolved_attrs)?;
            } else {
                // otherwise register and execute the handler
                let handler = StateChangeHandler {
                    string_with_varrefs_resolvers,
                    func: Box::new(set_value.clone()),
                    constant_values: resolved_attrs,
                    unresolved_attrs,
                };
                handler.run_with_state(&self.variables_state)?;
                window_state.put_handler(handler);
            }
        };
        if let Err(e) = result {
            eprintln!("Error resolving values: {:?}", e);
        }
    }
}

pub fn generate_string_with_var_refs_resolver(
    string_with_varrefs: value::StringWithVarRefs,
) -> impl Fn(&HashMap<VarName, PrimitiveValue>) -> PrimitiveValue {
    move |variables: &HashMap<VarName, PrimitiveValue>| {
        PrimitiveValue::from_string(
            string_with_varrefs
                .iter()
                .map(|entry| match entry {
                    value::StringOrVarRef::VarRef(var_name) => variables
                        .get(var_name)
                        .expect(&format!("Impossible state: unknown variable {}.\n{:?}", var_name, variables))
                        .clone()
                        .into_inner(),
                    value::StringOrVarRef::Primitive(s) => s.to_string(),
                })
                .join(""),
        )
    }
}

/// Run a command and get the output
pub fn run_command(cmd: &str) -> Result<PrimitiveValue> {
    let output = String::from_utf8(Command::new("/bin/sh").arg("-c").arg(cmd).output()?.stdout)?;
    let output = output.trim_matches('\n');
    Ok(PrimitiveValue::from(output))
}
