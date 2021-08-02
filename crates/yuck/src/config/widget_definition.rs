use std::collections::HashMap;

use simplexpr::SimplExpr;

use crate::{
    error::AstResult,
    parser::{
        ast::Ast,
        ast_iterator::AstIterator,
        from_ast::{FromAst, FromAstElementContent},
    },
};
use eww_shared_util::{AttrName, Span, VarName};

use super::widget_use::WidgetUse;
#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize)]
pub struct WidgetDefinition {
    pub name: String,
    pub expected_args: Vec<AttrName>,
    pub widget: WidgetUse,
    pub span: Span,
    pub args_span: Span,
}

impl FromAstElementContent for WidgetDefinition {
    const ELEMENT_NAME: &'static str = "defwidget";

    fn from_tail<I: Iterator<Item = Ast>>(span: Span, mut iter: AstIterator<I>) -> AstResult<Self> {
        let (_, name) = iter.expect_symbol()?;
        let (args_span, expected_args) = iter.expect_array()?;
        let expected_args = expected_args.into_iter().map(|x| x.as_symbol().map(AttrName)).collect::<AstResult<_>>()?;
        let widget = iter.expect_any().and_then(WidgetUse::from_ast)?;
        iter.expect_done()?;
        Ok(Self { name, expected_args, widget, span, args_span })
    }
}
