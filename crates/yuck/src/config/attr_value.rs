use std::collections::HashMap;

use eww_shared_util::VarName;
use simplexpr::{dynval::DynVal, eval::EvalError, SimplExpr};

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

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Action {
    Update(VarName, SimplExpr),
    Shell(SimplExpr),
    Noop,
}

impl Action {
    pub fn eval_exprs(&self, values: &HashMap<VarName, DynVal>) -> Result<ExecutableAction, EvalError> {
        Ok(match self {
            Self::Update(varname, expr) => ExecutableAction::Update(varname.clone(), expr.eval(values)?),
            Self::Shell(expr) => ExecutableAction::Shell(expr.eval(values)?.as_string()?),
            Self::Noop => ExecutableAction::Noop,
        })
    }

    pub fn collect_var_refs(&self) -> Vec<VarName> {
        match self {
            Self::Update(_, expr) => expr.collect_var_refs(),
            Self::Shell(expr) => expr.collect_var_refs(),
            Self::Noop => vec![],
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ExecutableAction {
    Update(VarName, DynVal),
    Shell(String),
    Noop,
}
