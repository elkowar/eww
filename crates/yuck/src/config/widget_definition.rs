use std::collections::HashMap;

use simplexpr::SimplExpr;

use crate::{
    error::{AstError::WrongExprType, AstResult, AstResultExt, FormFormatError},
    parser::{
        ast::Ast,
        ast_iterator::AstIterator,
        from_ast::{FromAst, FromAstElementContent},
    },
};
use eww_shared_util::{AttrName, Span, Spanned, VarName};

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
        let (name_span, name) = iter.expect_symbol().note(EXPECTED_WIDGET_DEF_FORMAT)?;
        let (args_span, expected_args) = iter
            .expect_array()
            .wrong_expr_type_to(|_, _| Some(FormFormatError::WidgetDefArglistMissing(name_span.point_span_at_end())))
            .note(EXPECTED_WIDGET_DEF_FORMAT)?;
        let expected_args = expected_args.into_iter().map(|x| x.as_symbol().map(AttrName)).collect::<AstResult<_>>()?;
        let widget = iter.expect_any().note(EXPECTED_WIDGET_DEF_FORMAT).and_then(WidgetUse::from_ast)?;
        iter.expect_done().map_err(|e| FormFormatError::WidgetDefMultipleChildren(e.span()))?;
        Ok(Self { name, expected_args, widget, span, args_span })
    }
}

static EXPECTED_WIDGET_DEF_FORMAT: &str = r#"Expected format: `(defwidget name [] (contained-widgets))`"#;
