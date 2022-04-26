use eww_shared_util::VarName;
use simplexpr::SimplExpr;

pub static ACTION_NAMES: &[&str] = &["update"];

// TODO: Maybe separate that into another file
#[derive(Debug, Clone)]
pub enum AttrValue {
    Action(Action),
    SimplExpr(SimplExpr),
}

#[derive(Debug, Clone)]
pub enum Action {
    Update(Update),
}

#[derive(Debug, Clone)]
pub struct Update {
    pub varname: VarName,
    pub value: SimplExpr,
}
