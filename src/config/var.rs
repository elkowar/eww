use std::collections::HashMap;

use simplexpr::{dynval::DynVal, SimplExpr};

use crate::{
    error::AstResult,
    parser::{
        ast::{Ast, AstIterator, Span},
        element::{Element, FromAst},
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

impl FromAst for VarDefinition {
    fn from_ast(e: Ast) -> AstResult<Self> {
        let span = e.span();
        spanned!(e.span(), {
            let list = e.as_list()?;
            let mut iter = AstIterator::new(list.into_iter());
            let _ = iter.expect_symbol()?;
            let (_, name) = iter.expect_symbol()?;
            let (_, initial_value) = iter.expect_value()?;
            Self { name: VarName(name), initial_value, span }
        })
    }
}
