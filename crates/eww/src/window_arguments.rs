use anyhow::{bail, Context, Result};
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

fn parse_value_from_args<T: FromStr>(name: &str, args: &mut HashMap<VarName, DynVal>) -> Result<Option<T>, T::Err> {
    args.remove(&VarName(name.to_string())).map(|x| FromStr::from_str(&x.as_string().unwrap())).transpose()
}

/// This stores the arguments given in the command line to create a window
/// While creating a window, we combine this with information from the
/// [`WindowDefinition`] to create a [WindowInitiator](`crate::window_initiator::WindowInitiator`), which stores all the
/// information required to start a window
#[derive(Debug, Clone)]
pub struct WindowArguments {
    /// Name of the window as defined in the eww config
    pub window_name: String,
    /// Instance ID of the window
    pub instance_id: String,
    pub anchor: Option<AnchorPoint>,
    pub args: HashMap<VarName, DynVal>,
    pub duration: Option<std::time::Duration>,
    pub monitor: Option<MonitorIdentifier>,
    pub pos: Option<Coords>,
    pub size: Option<Coords>,
}

impl WindowArguments {
    pub fn new_from_args(id: String, config_name: String, mut args: HashMap<VarName, DynVal>) -> Result<Self> {
        let initiator = WindowArguments {
            window_name: config_name,
            instance_id: id,
            pos: parse_value_from_args::<Coords>("pos", &mut args)?,
            size: parse_value_from_args::<Coords>("size", &mut args)?,
            monitor: parse_value_from_args::<MonitorIdentifier>("screen", &mut args)?,
            anchor: parse_value_from_args::<AnchorPoint>("anchor", &mut args)?,
            duration: parse_value_from_args::<DynVal>("duration", &mut args)?
                .map(|x| x.as_duration())
                .transpose()
                .context("Not a valid duration")?,
            args,
        };

        Ok(initiator)
    }

    /// Return a hashmap of all arguments the window was passed and expected, returning
    /// an error in case required arguments are missing or unexpected arguments are passed.
    pub fn get_local_window_variables(&self, window_def: &WindowDefinition) -> Result<HashMap<VarName, DynVal>> {
        let expected_args: HashSet<&String> = window_def.expected_args.iter().map(|x| &x.name.0).collect();
        let mut local_variables: HashMap<VarName, DynVal> = HashMap::new();

        // Ensure that the arguments passed to the window that are already interpreted by eww (id, screen)
        // are set to the correct values
        if expected_args.contains(&String::from("id")) {
            local_variables.insert(VarName::from("id"), DynVal::from(self.instance_id.clone()));
        }
        if self.monitor.is_some() && expected_args.contains(&String::from("screen")) {
            let mon_dyn = DynVal::from(&self.monitor.clone().unwrap());
            local_variables.insert(VarName::from("screen"), mon_dyn);
        }

        local_variables.extend(self.args.clone());

        for attr in &window_def.expected_args {
            let name = VarName::from(attr.name.clone());
            if !local_variables.contains_key(&name) && !attr.optional {
                bail!("Error, missing argument '{}' when creating window with id '{}'", attr.name, self.instance_id);
            }
        }

        if local_variables.len() != window_def.expected_args.len() {
            let unexpected_vars: Vec<_> = local_variables.keys().filter(|&n| !expected_args.contains(&n.0)).cloned().collect();
            bail!(
                "variables {} unexpectedly defined when creating window with id '{}'",
                unexpected_vars.join(", "),
                self.instance_id,
            );
        }

        Ok(local_variables)
    }
}
