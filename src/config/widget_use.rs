use std::collections::HashMap;

use simplexpr::SimplExpr;

use crate::{
    error::AstResult,
    parser::{
        ast::{Ast, AstIterator, Span},
        from_ast::FromAst,
    },
    spanned,
    value::AttrName,
};
#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize)]
pub struct WidgetUse {
    pub name: String,
    pub attrs: HashMap<AttrName, SimplExpr>,
    pub children: Vec<WidgetUse>,
    pub span: Span,
}

impl FromAst for WidgetUse {
    fn from_ast(e: Ast) -> AstResult<Self> {
        let span = e.span();
        spanned!(e.span(), {
            if let Ok(text) = e.as_literal_ref() {
                Self {
                    name: "text".to_string(),
                    attrs: maplit::hashmap! { AttrName("text".to_string()) => SimplExpr::Literal(span.into(), text.clone()) },
                    children: Vec::new(),
                    span,
                }
            } else {
                let list = e.as_list()?;
                let mut iter = AstIterator::new(list.into_iter());
                let (_, name) = iter.expect_symbol()?;
                let attrs = iter.expect_key_values()?.into_iter().map(|(k, v)| (AttrName(k), v)).collect();
                let children = iter.map(WidgetUse::from_ast).collect::<AstResult<Vec<_>>>()?;
                Self { name, attrs, children, span }
            }
        })
    }
}
