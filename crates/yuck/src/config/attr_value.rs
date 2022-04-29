use std::collections::HashMap;

use eww_shared_util::VarName;
use serde::{Deserialize, Serialize};
use simplexpr::{
    dynval::{DynVal, Opaque, OpaqueType},
    eval::EvalError,
    SimplExpr,
};

pub static ACTION_NAMES: &[&str] = &["update"];

#[derive(Debug, Clone)]
pub enum AttrValue {
    Action(Action),
    SimplExpr(SimplExpr),
}

impl AttrValue {
    pub fn try_into_simplexpr(&self) -> Option<&SimplExpr> {
        match self {
            Self::SimplExpr(x) => Some(x),
            _ => None,
        }
    }

    pub fn collect_var_refs(&self) -> Vec<VarName> {
        match self {
            Self::SimplExpr(expr) => expr.collect_var_refs(),
            Self::Action(action) => action.collect_var_refs(),
        }
    }
}

/// an action as it is provided by the user. These actions contain Expressions which may reference variables.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub enum Action {
    Update(VarName, SimplExpr),
    Shell(SimplExpr),
    Noop,
}

impl OpaqueType for Action {
    const TYPE_NAME: &'static str = "action";
}

impl Action {
    pub fn resolve_to_executable(self, values: &HashMap<VarName, DynVal>) -> ExecutableAction {
        match self {
            Self::Update(varname, expr) => ExecutableAction::Update(varname, expr.resolve_refs_lenient(values)),
            Self::Shell(expr) => ExecutableAction::Shell(expr.resolve_refs_lenient(values)),
            Self::Noop => ExecutableAction::Noop,
        }
    }

    // TODO the special case for event here is super ugly
    /// Returns all variable references in this action, EXCEPT a variable called "event",
    /// as that variable is specifically filled in when evaluating the event.
    /// see [`eww::widgets::run_action`]
    pub fn collect_var_refs(&self) -> Vec<VarName> {
        let refs = match self {
            Self::Update(_, expr) => expr.collect_var_refs(),
            Self::Shell(expr) => expr.collect_var_refs(),
            Self::Noop => vec![],
        };
        refs.into_iter().filter(|x| x.0 != "event").collect()
    }
}

/// an action ready for execution.
/// The expressions in this struct may only reference the variable "event", and _must_ be fully resolved otherwise.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ExecutableAction {
    Update(VarName, SimplExpr),
    Shell(SimplExpr),
    Noop,
}
