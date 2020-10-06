use crate::value::VarName;
use anyhow::*;
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;

use crate::value::{AttrValue, PrimitiveValue};

//pub struct StateChangeHandler(Box<dyn Fn(HashMap<String, PrimitiveValue>) + 'static>);
pub struct StateChangeHandler {
    func: Box<dyn Fn(HashMap<String, PrimitiveValue>) -> Result<()> + 'static>,
    constant_values: HashMap<String, PrimitiveValue>,
    unresolved_attrs: HashMap<String, VarName>,
}

impl StateChangeHandler {
    fn run_with_state(&self, state: &HashMap<VarName, PrimitiveValue>) -> Result<()> {
        let mut all_resolved_attrs = self.constant_values.clone();
        for (attr_name, var_ref) in self.unresolved_attrs.iter() {
            let resolved = state
                .get(var_ref)
                // TODO provide context here, including line numbers
                .with_context(|| format!("Unknown variable '{}' was referenced", var_ref))?;
            all_resolved_attrs.insert(attr_name.to_owned(), resolved.clone());
        }

        let result: Result<_> = (self.func)(all_resolved_attrs);
        if let Err(err) = result {
            eprintln!("WARN: Error while resolving attributes: {}", err);
        }

        Ok(())
    }
}

pub struct StateChangeHandlers {
    handlers: HashMap<VarName, Vec<Arc<StateChangeHandler>>>,
}

impl StateChangeHandlers {
    fn put_handler(&mut self, handler: StateChangeHandler) {
        let handler = Arc::new(handler);
        for var_name in handler.unresolved_attrs.values() {
            let entry: &mut Vec<Arc<StateChangeHandler>> = self.handlers.entry(var_name.clone()).or_insert_with(Vec::new);
            entry.push(handler.clone());
        }
    }

    fn get(&self, key: &VarName) -> Option<&Vec<Arc<StateChangeHandler>>> {
        self.handlers.get(key)
    }

    fn clear(&mut self) {
        self.handlers.clear();
    }
}

pub struct EwwState {
    state_change_handlers: StateChangeHandlers,
    state: HashMap<VarName, PrimitiveValue>,
}

impl Default for EwwState {
    fn default() -> Self {
        EwwState {
            state_change_handlers: StateChangeHandlers {
                handlers: HashMap::new(),
            },
            state: HashMap::new(),
        }
    }
}

impl std::fmt::Debug for EwwState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EwwState {{ state: {:?} }}", self.state)
    }
}

impl EwwState {
    pub fn from_default_vars(defaults: HashMap<VarName, PrimitiveValue>) -> Self {
        EwwState {
            state: defaults,
            ..EwwState::default()
        }
    }

    pub fn clear_callbacks(&mut self) {
        self.state_change_handlers.clear();
    }

    pub fn update_value(&mut self, key: VarName, value: PrimitiveValue) -> Result<()> {
        if let Some(handlers) = self.state_change_handlers.get(&key) {
            self.state.insert(key.clone(), value);
            for handler in handlers {
                handler
                    .run_with_state(&self.state)
                    .with_context(|| format!("When updating value of {}", &key))?;
            }
        }
        Ok(())
    }

    pub fn resolve<F: Fn(HashMap<String, PrimitiveValue>) -> Result<()> + 'static + Clone>(
        &mut self,
        local_env: &HashMap<VarName, AttrValue>,
        mut needed_attributes: HashMap<String, AttrValue>,
        set_value: F,
    ) {
        let mut resolved_attrs = HashMap::new();
        let mut unresolved_attrs: HashMap<String, VarName> = HashMap::new();
        needed_attributes
            .drain()
            .for_each(|(attr_name, attr_value)| match attr_value {
                AttrValue::Concrete(primitive) => {
                    resolved_attrs.insert(attr_name, primitive);
                }
                AttrValue::VarRef(var_name) => match local_env.get(&var_name) {
                    Some(AttrValue::VarRef(var_ref_from_local)) => {
                        unresolved_attrs.insert(attr_name, var_ref_from_local.clone());
                    }
                    Some(AttrValue::Concrete(concrete_from_local)) => {
                        resolved_attrs.insert(attr_name, concrete_from_local.clone());
                    }
                    None => {
                        unresolved_attrs.insert(attr_name, var_name);
                    }
                },
            });

        let result: Result<_> = try {
            if unresolved_attrs.is_empty() {
                set_value(resolved_attrs)?;
            } else {
                let handler = StateChangeHandler {
                    func: Box::new(set_value.clone()),
                    constant_values: resolved_attrs,
                    unresolved_attrs,
                };
                handler.run_with_state(&self.state)?;
                self.state_change_handlers.put_handler(handler);
            }
        };
        if let Err(e) = result {
            eprintln!("{}", e);
        }
    }
}

pub fn run_command(cmd: &str) -> Result<PrimitiveValue> {
    let output = String::from_utf8(Command::new("/bin/bash").arg("-c").arg(cmd).output()?.stdout)?;
    let output = output.trim_matches('\n');
    Ok(PrimitiveValue::from(output))
}

pub fn recursive_lookup<'a>(data: &'a HashMap<VarName, AttrValue>, key: &VarName) -> Result<&'a PrimitiveValue> {
    match data.get(key) {
        Some(AttrValue::Concrete(x)) => Ok(x),
        Some(AttrValue::VarRef(x)) => recursive_lookup(data, x),
        None => Err(anyhow!("No value found for key '{}'", key)),
    }
}
