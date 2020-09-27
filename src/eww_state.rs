use anyhow::*;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::process::Command;

use crate::value::{AttrValue, CommandPollingUse, PrimitiveValue};

#[derive(Default)]
pub struct EwwState {
    on_change_handlers: HashMap<String, Vec<Box<dyn Fn(PrimitiveValue) + 'static>>>,
    polling_commands: Vec<(CommandPollingUse, Box<dyn Fn(PrimitiveValue) + 'static>)>,
    state: HashMap<String, PrimitiveValue>,
}

impl std::fmt::Debug for EwwState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EwwState {{ state: {:?} }}", self.state)
    }
}

impl EwwState {
    pub fn from_default_vars(defaults: HashMap<String, PrimitiveValue>) -> Self {
        EwwState {
            state: defaults,
            ..EwwState::default()
        }
    }
    pub fn update_value(&mut self, key: String, value: PrimitiveValue) {
        if let Some(handlers) = self.on_change_handlers.get(&key) {
            for on_change in handlers {
                on_change(value.clone());
            }
        }
        self.state.insert(key, value);
    }

    pub fn resolve<F: Fn(PrimitiveValue) + 'static + Clone>(
        &mut self,
        local_env: &HashMap<String, AttrValue>,
        value: &AttrValue,
        set_value: F,
    ) -> bool {
        match value {
            AttrValue::VarRef(name) => {
                // get value from globals
                if let Some(value) = self.state.get(name).cloned() {
                    self.on_change_handlers
                        .entry(name.to_string())
                        .or_insert_with(Vec::new)
                        .push(Box::new(set_value.clone()));
                    self.resolve(local_env, &value.into(), set_value)
                } else if let Some(value) = local_env.get(name).cloned() {
                    // get value from local
                    self.resolve(local_env, &value, set_value)
                } else {
                    false
                }
            }
            AttrValue::CommandPolling(command_polling_use) => {
                self.polling_commands
                    .push((command_polling_use.clone(), Box::new(set_value.clone())));
                // TODO how do i handle commands needing to be run on the first resolve? this is an issue,....
                //self.resolve(local_env, &value.into(), set_value);
                true
            }
            AttrValue::Concrete(value) => {
                set_value(value.clone());
                true
            }
        }
    }

    pub fn resolve_into<TE: std::fmt::Debug, V: TryFrom<PrimitiveValue, Error = TE>, F: Fn(V) + 'static + Clone>(
        &mut self,
        local_env: &HashMap<String, AttrValue>,
        value: &AttrValue,
        set_value: F,
    ) -> bool {
        self.resolve(local_env, value, move |x| {
            if let Err(e) = x.try_into().map(|v| set_value(v)) {
                eprintln!("error while resolving value: {:?}", e);
            };
        })
    }
    pub fn resolve_f64<F: Fn(f64) + 'static + Clone>(
        &mut self,
        local_env: &HashMap<String, AttrValue>,
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
        local_env: &HashMap<String, AttrValue>,
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
        local_env: &HashMap<String, AttrValue>,
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
    Ok(PrimitiveValue::from(output))
}
