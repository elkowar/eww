use simplexpr::SimplExpr;

use crate::{
    config::attributes::AttrEntry,
    error::{DiagError, DiagResult, DiagResultExt},
    gen_diagnostic,
    parser::{
        ast::Ast,
        ast_iterator::AstIterator,
        from_ast::{FromAst, FromAstElementContent},
    },
};
use eww_shared_util::{AttrName, Span, Spanned, VarName};

use super::attributes::Attributes;

#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize)]
pub enum WidgetUse {
    Basic(BasicWidgetUse),
    Loop(LoopWidgetUse),
    Children(ChildrenWidgetUse),
}

#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize)]
pub struct LoopWidgetUse {
    pub element_name: VarName,
    pub elements_expr: SimplExpr,
    pub elements_expr_span: Span,
    pub body: Box<WidgetUse>,
    pub span: Span,
}

#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize)]
pub struct ChildrenWidgetUse {
    pub span: Span,
    pub nth_expr: Option<SimplExpr>,
}

#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize)]
pub struct BasicWidgetUse {
    pub name: String,
    pub attrs: Attributes,
    pub children: Vec<WidgetUse>,
    pub span: Span,
    pub name_span: Span,
}

impl BasicWidgetUse {
    pub fn children_span(&self) -> Span {
        if self.children.is_empty() {
            self.span.point_span_at_end().shifted(-1)
        } else {
            self.children.first().unwrap().span().to(self.children.last().unwrap().span())
        }
    }

    fn from_iter<I: Iterator<Item = Ast>>(
        span: Span,
        name: String,
        name_span: Span,
        mut iter: AstIterator<I>,
    ) -> DiagResult<Self> {
        let attrs = iter.expect_key_values()?;
        let children = iter.map(WidgetUse::from_ast).collect::<DiagResult<Vec<_>>>()?;
        Ok(Self { name, attrs, children, span, name_span })
    }
}

impl FromAstElementContent for LoopWidgetUse {
    const ELEMENT_NAME: &'static str = "for";

    fn from_tail<I: Iterator<Item = Ast>>(span: Span, mut iter: AstIterator<I>) -> DiagResult<Self> {
        let (_element_name_span, element_name) = iter.expect_symbol()?;
        let (in_string_span, in_string) = iter.expect_symbol()?;
        if in_string != "in" {
            return Err(DiagError(gen_diagnostic! {
                msg = "Expected 'in' in this position, but got '{in_string}'",
                label = in_string_span
            }));
        }
        let (elements_span, elements_expr) = iter.expect_simplexpr()?;
        let body = iter.expect_any().map_err(DiagError::from).note("Expected a loop body").and_then(WidgetUse::from_ast)?;
        iter.expect_done()?;
        Ok(Self {
            element_name: VarName(element_name),
            elements_expr,
            body: Box::new(body),
            span,
            elements_expr_span: elements_span,
        })
    }
}

impl FromAstElementContent for ChildrenWidgetUse {
    const ELEMENT_NAME: &'static str = "children";

    fn from_tail<I: Iterator<Item = Ast>>(span: Span, mut iter: AstIterator<I>) -> DiagResult<Self> {
        let mut attrs = iter.expect_key_values()?;
        let nth_expr = attrs.ast_optional("nth")?;
        iter.expect_done()?;
        Ok(Self { span, nth_expr })
    }
}

impl FromAst for WidgetUse {
    fn from_ast(e: Ast) -> DiagResult<Self> {
        let span = e.span();
        if let Ok(value) = e.clone().as_simplexpr() {
            Ok(WidgetUse::Basic(label_from_simplexpr(value, span)))
        } else {
            let mut iter = e.try_ast_iter()?;
            let (name_span, name) = iter.expect_symbol()?;
            match name.as_ref() {
                LoopWidgetUse::ELEMENT_NAME => Ok(WidgetUse::Loop(LoopWidgetUse::from_tail(span, iter)?)),
                ChildrenWidgetUse::ELEMENT_NAME => Ok(WidgetUse::Children(ChildrenWidgetUse::from_tail(span, iter)?)),
                _ => Ok(WidgetUse::Basic(BasicWidgetUse::from_iter(span, name, name_span, iter)?)),
            }
        }
    }
}

fn label_from_simplexpr(value: SimplExpr, span: Span) -> BasicWidgetUse {
    BasicWidgetUse {
        name: "label".to_string(),
        name_span: span.point_span(),
        attrs: Attributes::new(
            span,
            maplit::hashmap! {
                AttrName("text".to_string()) => AttrEntry::new(
                    span,
                    Ast::SimplExpr(span, value)
                )
            },
        ),
        children: Vec::new(),
        span,
    }
}

macro_rules! impl_spanned {
    ($($super:ident => $name:ident),*) => {
        $(impl Spanned for $name { fn span(&self) -> Span { self.span } })*
        impl Spanned for WidgetUse {
            fn span(&self) -> Span {
                match self { $(WidgetUse::$super(widget) => widget.span),* }
            }
        }
    }
}
impl_spanned!(Basic => BasicWidgetUse, Loop => LoopWidgetUse, Children => ChildrenWidgetUse);
