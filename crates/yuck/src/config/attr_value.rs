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
