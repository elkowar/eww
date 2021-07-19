use std::collections::HashMap;

use simplexpr::{dynval::DynVal, SimplExpr};

use crate::{
    error::AstResult,
    parser::{
        ast::{Ast, AstIterator, Span},
        element::{Element, FromAst, FromAstElementContent},
    },
    spanned,
    value::{AttrName, VarName},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct VarDefinition {
    pub name: VarName,
    pub initial_value: DynVal,
    pub span: Span,
}

impl FromAstElementContent for VarDefinition {
    fn get_element_name() -> &'static str {
        "defvar"
    }

    fn from_tail<I: Iterator<Item = Ast>>(span: Span, mut iter: AstIterator<I>) -> AstResult<Self> {
        let (_, name) = iter.expect_symbol()?;
        let (_, initial_value) = iter.expect_value()?;
        Ok(Self { name: VarName(name), initial_value, span })
    }
}
