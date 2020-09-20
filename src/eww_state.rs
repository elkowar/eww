use std::collections::HashMap;

use crate::value::{AttrValue, PrimitiveValue};

#[derive(Default)]
pub struct EwwState {
    on_change_handlers: HashMap<String, Vec<Box<dyn Fn(PrimitiveValue) + 'static>>>,
    state: HashMap<String, PrimitiveValue>,
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
        dbg!("resolve: ", value);
        match value {
            AttrValue::VarRef(name) => {
                if let Some(value) = self.state.get(name).cloned() {
                    self.on_change_handlers
                        .entry(name.to_string())
                        .or_insert_with(Vec::new)
                        .push(Box::new(set_value.clone()));
                    self.resolve(local_env, &value.into(), set_value)
                } else if let Some(value) = local_env.get(name).cloned() {
                    self.resolve(local_env, &value, set_value)
                } else {
                    false
                }
            }
            AttrValue::Concrete(value) => {
                set_value(value.clone());
                true
            }
        }
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
    pub fn resolve_string<F: Fn(String) + 'static + Clone>(
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
