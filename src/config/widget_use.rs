use std::collections::HashMap;

use simplexpr::SimplExpr;

use crate::{
    config::attributes::AttrEntry,
    error::AstResult,
    parser::{
        ast::{Ast, AstIterator, Span},
        from_ast::FromAst,
    },
    spanned,
    value::AttrName,
};

use super::attributes::Attributes;

#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize)]
pub struct WidgetUse {
    pub name: String,
    pub attrs: Attributes,
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
                    attrs: Attributes::new(
                        span.into(),
                        maplit::hashmap! {
                            AttrName("text".to_string()) => AttrEntry::new(
                                span.into(),
                                SimplExpr::Literal(span.into(), text.clone())
                            )
                        },
                    ),
                    children: Vec::new(),
                    span,
                }
            } else {
                let mut iter = e.try_ast_iter()?;
                let (_, name) = iter.expect_symbol()?;
                let attrs = iter.expect_key_values()?;
                let children = iter.map(WidgetUse::from_ast).collect::<AstResult<Vec<_>>>()?;
                Self { name, attrs, children, span }
            }
        })
    }
}
