use crate::{
    error::{DiagError, DiagResult, DiagResultExt},
    format_diagnostic::ToDiagnostic,
    gen_diagnostic,
    parser::{
        ast::Ast,
        ast_iterator::AstIterator,
        from_ast::{FromAst, FromAstElementContent},
    },
};
use eww_shared_util::{Span, Spanned};

use super::{attributes::AttrSpec, widget_use::WidgetUse};

#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize)]
pub struct WidgetDefinition {
    pub name: String,
    pub expected_args: Vec<AttrSpec>,
    pub widget: WidgetUse,
    pub span: Span,
    pub args_span: Span,
}

impl FromAstElementContent for WidgetDefinition {
    const ELEMENT_NAME: &'static str = "defwidget";

    fn from_tail<I: Iterator<Item = Ast>>(span: Span, mut iter: AstIterator<I>) -> DiagResult<Self> {
        let (name_span, name) = iter.expect_symbol().map_err(DiagError::from).note(EXPECTED_WIDGET_DEF_FORMAT)?;
        let (args_span, expected_args) = iter
            .expect_array()
            .map_err(|e| {
                DiagError(match e {
                    crate::ast_error::AstError::WrongExprType(..) => gen_diagnostic! {
                        msg = "Widget definition missing argument list",
                        label = name_span.point_span_at_end() => "Insert the argument list (e.g.: `[]`) here",
                        note = "This list needs to declare all the non-global variables / attributes used in this widget."
                    },
                    other => other.to_diagnostic(),
                })
            })
            .note(EXPECTED_WIDGET_DEF_FORMAT)?;
        let expected_args = expected_args.into_iter().map(AttrSpec::from_ast).collect::<DiagResult<_>>()?;
        let widget = iter.expect_any().map_err(DiagError::from).note(EXPECTED_WIDGET_DEF_FORMAT).and_then(WidgetUse::from_ast)?;
        iter.expect_done().map_err(|e| {
            DiagError(gen_diagnostic! {
                msg = "Widget definition has more than one child widget",
                label = e.span() => "Found more than one child element here.",
                note = "A widget-definition may only contain one child element.\n\
                        To include multiple elements, wrap these elements in a single container widget such as `box`.\n\
                        This is necessary as eww can't know how you want these elements to be layed out otherwise."
            })
        })?;

        Ok(Self { name, expected_args, widget, span, args_span })
    }
}

static EXPECTED_WIDGET_DEF_FORMAT: &str = r#"Expected format: `(defwidget name [] (contained-widgets))`"#;
