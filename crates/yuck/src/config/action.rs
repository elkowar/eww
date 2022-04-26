use std::collections::HashMap;

use eww_shared_util::VarName;
use simplexpr::{dynval::DynVal, eval::EvalError, SimplExpr};

pub static ACTION_NAMES: &[&str] = &["update"];

// TODO: Maybe separate that into another file
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
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Action {
    Update(VarName, SimplExpr),
    Noop,
}

impl Action {
    pub fn eval_exprs(&self, values: &HashMap<VarName, DynVal>) -> Result<ResolvedAction, EvalError> {
        Ok(match self {
            Self::Update(varname, expr) => ResolvedAction::Update(varname.clone(), expr.eval(values)?),
            Self::Noop => ResolvedAction::Noop,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ResolvedAction {
    Update(VarName, DynVal),
    Noop,
}
