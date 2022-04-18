use anyhow::{anyhow, Context, Result};
use eww_shared_util::VarName;
use simplexpr::dynval::DynVal;
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};
use yuck::{
    config::{monitor::MonitorIdentifier, window_definition::WindowDefinition, window_geometry::AnchorPoint},
    value::Coords,
};

fn extract_value_from_args(name: &str, args: &mut Vec<(VarName, DynVal)>) -> Option<DynVal> {
    let var_name = name.to_string();
    let pos = args.iter().position(|(n, _)| n.0 == var_name);

    if let Some(unwrapped_pos) = pos {
        let (_, val) = args.remove(unwrapped_pos);
        Some(val)
    } else {
        None
    }
}

fn parse_value_from_args<T: FromStr>(name: &str, args: &mut Vec<(VarName, DynVal)>) -> Result<Option<T>, T::Err> {
    extract_value_from_args(name, args).map(|x| FromStr::from_str(&x.as_string().unwrap())).transpose()
}

/// This stores the arguments given in the command line to create a window
/// While creating a window, we combine this with information from the
/// WindowDefinition to create a WindowInitiator, which stores all the
/// information required to start a window
#[derive(Debug, Clone)]
pub struct WindowArguments {
    pub anchor: Option<AnchorPoint>,
    pub args: Vec<(VarName, DynVal)>,
    pub config_name: String,
    pub duration: Option<std::time::Duration>,
    pub id: String,
    pub monitor: Option<MonitorIdentifier>,
    pub pos: Option<Coords>,
    pub size: Option<Coords>,
}

impl WindowArguments {
    pub fn new(
        id: String,
        config_name: String,
        pos: Option<Coords>,
        size: Option<Coords>,
        monitor: Option<MonitorIdentifier>,
        anchor: Option<AnchorPoint>,
        duration: Option<std::time::Duration>,
        args: Vec<(VarName, DynVal)>,
    ) -> Self {
        WindowArguments { id, config_name, pos, size, monitor, anchor, duration, args }
    }

    pub fn new_from_args(id: String, config_name: String, mut args: Vec<(VarName, DynVal)>) -> Result<Self> {
        let initiator = WindowArguments {
            config_name,
            id,
            pos: parse_value_from_args::<Coords>("pos", &mut args)?,
            size: parse_value_from_args::<Coords>("size", &mut args)?,
            monitor: parse_value_from_args::<MonitorIdentifier>("screen", &mut args)?,
            anchor: parse_value_from_args::<AnchorPoint>("anchor", &mut args)?,
            duration: extract_value_from_args("duration", &mut args)
                .map(|x| x.as_duration())
                .transpose()
                .context("Not a valid duration")?,
            args,
        };

        Ok(initiator)
    }

    pub fn get_local_window_variables(&self, window_def: &WindowDefinition) -> Result<HashMap<VarName, DynVal>> {
        let expected_args: HashSet<&String> = window_def.expected_args.iter().map(|x| &x.name.0).collect();
        let mut local_variables: HashMap<VarName, DynVal> = HashMap::new();

        // Inserts these first so they can be overridden
        if expected_args.contains(&"id".to_string()) {
            local_variables.insert(VarName::from("id"), DynVal::from(self.id.clone()));
        }
        if self.monitor.is_some() && expected_args.contains(&"screen".to_string()) {
            let mon_dyn = self.monitor.clone().unwrap().to_dynval();
            local_variables.insert(VarName::from("screen"), mon_dyn);
        }

        local_variables.extend(self.args.clone());

        for attr in &window_def.expected_args {
            let name = VarName::from(attr.name.clone());

            // This is here to get around the map_entry warning
            let mut inserted = false;
            local_variables.entry(name).or_insert_with(|| {
                inserted = true;
                DynVal::from_string(String::new())
            });

            if inserted && !attr.optional {
                return Err(anyhow!("Error, {} was required when creating {} but was not given", attr.name, self.config_name));
            }
        }

        if local_variables.len() != window_def.expected_args.len() {
            let unexpected_vars: Vec<VarName> = local_variables
                .iter()
                .filter_map(|(n, _)| if !expected_args.contains(&n.0) { Some(n.clone()) } else { None })
                .collect();
            return Err(anyhow!(
                "'{}' {} unexpectedly defined when creating window {}",
                unexpected_vars.join(","),
                if unexpected_vars.len() == 1 { "was" } else { "were" },
                self.config_name
            ));
        }

        Ok(local_variables)
    }
}
