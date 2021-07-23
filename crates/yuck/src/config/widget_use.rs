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
    pub name_span: Span,
}

impl FromAst for WidgetUse {
    fn from_ast(e: Ast) -> AstResult<Self> {
        let span = e.span();
        if let Ok(value) = e.clone().as_simplexpr() {
            Ok(Self {
                name: "label".to_string(),
                name_span: span.point_span(),
                attrs: Attributes::new(
                    span,
                    maplit::hashmap! {
                        AttrName("text".to_string()) => AttrEntry::new(
                            span,
                            Ast::SimplExpr(span.into(), value.clone())
                        )
                    },
                ),
                children: Vec::new(),
                span,
            })
        } else {
            let mut iter = e.try_ast_iter()?;
            let (name_span, name) = iter.expect_symbol()?;
            let attrs = iter.expect_key_values()?;
            let children = iter.map(WidgetUse::from_ast).collect::<AstResult<Vec<_>>>()?;
            Ok(Self { name, attrs, children, span, name_span })
        }
    }
}
