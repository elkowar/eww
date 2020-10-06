use crate::value::VarName;
use anyhow::*;
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;

use crate::value::{AttrValue, PrimitiveValue};

//pub struct StateChangeHandler(Box<dyn Fn(HashMap<String, PrimitiveValue>) + 'static>);

pub struct StateChangeHandlers {
    handlers: HashMap<VarName, Vec<Arc<dyn Fn(HashMap<String, PrimitiveValue>) + 'static>>>,
}

impl StateChangeHandlers {
    fn put_handler(&mut self, var_names: Vec<VarName>, handler: Arc<dyn Fn(HashMap<String, PrimitiveValue>) + 'static>) {
        for var_name in var_names {
            let entry: &mut Vec<Arc<dyn Fn(HashMap<String, PrimitiveValue>) + 'static>> =
                self.handlers.entry(var_name).or_insert_with(Vec::new);
            entry.push(handler);
        }
    }
}

pub struct EwwState {
    state_change_handlers: StateChangeHandlers,
    //on_change_handlers: HashMap<VarName, Vec<StateChangeHandler>>,
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
        self.on_change_handlers.clear();
    }

    pub fn update_value(&mut self, key: VarName, value: PrimitiveValue) {
        if let Some(handlers) = self.on_change_handlers.get(&key) {
            for on_change in handlers {
                on_change(value.clone());
            }
        }
        self.state.insert(key, value);
    }

    pub fn resolve<F: Fn(PrimitiveValue) + 'static + Clone>(
        &mut self,
        local_env: &HashMap<VarName, AttrValue>,
        value: &AttrValue,
        set_value: F,
    ) -> bool {
        match value {
            AttrValue::VarRef(name) => {
                // get value from globals
                if let Some(value) = self.state.get(&name).cloned() {
                    self.on_change_handlers
                        .entry(name.clone())
                        .or_insert_with(Vec::new)
                        .push(Box::new(set_value.clone()));
                    self.resolve(local_env, &value.into(), set_value)
                } else if let Some(value) = local_env.get(&name).cloned() {
                    // get value from local
                    self.resolve(local_env, &value, set_value)
                } else {
                    eprintln!("WARN: unknown variable '{}' was referenced", name);
                    false
                }
            }
            AttrValue::Concrete(value) => {
                set_value(value.clone());
                true
            }
        }
    }

    //pub fn resolve_attrs<F: Fn(HashMap<String, PrimitiveValue>) + 'static + Clone>(
    //&mut self,
    //local_env: &HashMap<VarName, AttrValue>,
    //unresolved_attrs: HashMap<String, AttrValue>,
    //state_update_handler: F,
    //) {
    //let var_names = values.iter().filter_map(|value| value.as_var_ref().ok()).collect();
    //self.state_change_handlers
    //.put_handler(var_names, Arc::new(state_update_handler))
    //}

    pub fn resolve_f64<F: Fn(f64) + 'static + Clone>(
        &mut self,
        local_env: &HashMap<VarName, AttrValue>,
        value: &AttrValue,
        set_value: F,
    ) -> bool {
        self.resolve(local_env, value, move |x| {
            if let Err(e) = x.as_f64().map(|v| set_value(v)) {
                eprintln!("error while resolving value: {}", e);
            };
        })
    }

    #[allow(dead_code)]
    pub fn resolve_bool<F: Fn(bool) + 'static + Clone>(
        &mut self,
        local_env: &HashMap<VarName, AttrValue>,
        value: &AttrValue,
        set_value: F,
    ) -> bool {
        self.resolve(local_env, value, move |x| {
            if let Err(e) = x.as_bool().map(|v| set_value(v)) {
                eprintln!("error while resolving value: {}", e);
            };
        })
    }
    pub fn resolve_str<F: Fn(String) + 'static + Clone>(
        &mut self,
        local_env: &HashMap<VarName, AttrValue>,
        value: &AttrValue,
        set_value: F,
    ) -> bool {
        self.resolve(local_env, value, move |x| {
            if let Err(e) = x.as_string().map(|v| set_value(v.clone())) {
                eprintln!("error while resolving value: {}", e);
            };
        })
    }
}

pub fn run_command(cmd: &str) -> Result<PrimitiveValue> {
    let output = String::from_utf8(Command::new("/bin/bash").arg("-c").arg(cmd).output()?.stdout)?;
    let output = output.trim_matches('\n');
    Ok(PrimitiveValue::from(output))
}
