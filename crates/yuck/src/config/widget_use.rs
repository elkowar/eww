use std::collections::HashMap;

use simplexpr::SimplExpr;

use crate::{
    config::attributes::AttrEntry,
    error::AstResult,
    parser::{ast::Ast, ast_iterator::AstIterator, from_ast::FromAst},
};
use eww_shared_util::{AttrName, Span, VarName};

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
        if let Ok(text) = e.as_literal_ref() {
            Ok(Self {
                name: "label".to_string(),
                attrs: Attributes::new(
                    span.into(),
                    maplit::hashmap! {
                        AttrName("text".to_string()) => AttrEntry::new(
                            span.into(),
                            Ast::Literal(span.into(), text.clone())
                        )
                    },
                ),
                children: Vec::new(),
                span,
            })
        } else {
            let mut iter = e.try_ast_iter()?;
            let (_, name) = iter.expect_symbol()?;
            let attrs = iter.expect_key_values()?;
            let children = iter.map(WidgetUse::from_ast).collect::<AstResult<Vec<_>>>()?;
            Ok(Self { name, attrs, children, span })
        }
    }
}
