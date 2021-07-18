use std::collections::HashMap;

use simplexpr::SimplExpr;

use crate::{
    error::AstResult,
    parser::{
        ast::{Ast, AstIterator, Span},
        element::{Element, FromAst},
    },
    spanned,
    value::{AttrName, VarName},
};

use super::widget_use::WidgetUse;
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct WidgetDefinition {
    pub name: String,
    pub expected_args: Vec<AttrName>,
    pub widget: WidgetUse,
    pub span: Span,
    pub args_span: Span,
}

impl FromAst for WidgetDefinition {
    fn from_ast(e: Ast) -> AstResult<Self> {
        let span = e.span();
        spanned!(e.span(), {
            let list = e.as_list()?;
            let mut iter = AstIterator::new(list.into_iter());

            let (_, def_type) = iter.expect_symbol()?;
            assert!(def_type == "defwidget");

            let (_, name) = iter.expect_symbol()?;
            let (args_span, expected_args) = iter.expect_array()?;
            let expected_args = expected_args.into_iter().map(|x| x.as_symbol().map(AttrName)).collect::<AstResult<_>>()?;
            let widget = iter.expect_any().and_then(WidgetUse::from_ast)?;
            // TODO verify that this was the last element in the list
            // iter.expect_done()?;
            Self { name, expected_args, widget, span, args_span }
        })
    }
}
