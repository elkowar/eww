use simplexpr::{ast::SimplExpr, dynval::DynVal};
use std::collections::HashMap;

use super::ast::{Ast, AstType};
use crate::{
    ast_error::AstError,
    config::attributes::{AttrEntry, Attributes},
};
use eww_shared_util::{AttrName, Span, Spanned, VarName};

/// Iterator over [`crate::parser::ast::Ast`] nodes which allows to explicitly expect specific types of items
pub struct AstIterator<I: Iterator<Item = Ast>> {
    remaining_span: Span,
    iter: itertools::PutBack<I>,
}

macro_rules! return_or_put_back {
    ($(fn $name:ident -> $expr_type:expr, $t:ty = $p:pat => $ret:expr)*) => {
        $(
            pub fn $name(&mut self) -> Result<$t, AstError> {
                let expr_type = $expr_type;
                use eww_shared_util::Spanned;
                match self.expect_any()? {
                    $p => Ok($ret),
                    other => {
                        let span = other.span();
                        let actual_type = other.expr_type();
                        self.put_back(other);
                        Err(AstError::WrongExprType(span, expr_type, actual_type))
                    }
                }
            }
        )*
    };
}

impl<I: Iterator<Item = Ast>> AstIterator<I> {
    return_or_put_back! {
        fn expect_symbol    -> AstType::Symbol,    (Span, String)    = Ast::Symbol(span, x)    => (span, x)
        fn expect_list      -> AstType::List,      (Span, Vec<Ast>)  = Ast::List(span, x)      => (span, x)
        fn expect_array     -> AstType::Array,     (Span, Vec<Ast>)  = Ast::Array(span, x)     => (span, x)
    }

    pub fn expect_literal(&mut self) -> Result<(Span, DynVal), AstError> {
        // TODO add some others
        match self.expect_any()? {
            // Ast::Array(_, _) => todo!(),
            Ast::SimplExpr(span, expr) => Ok((span, expr.eval_no_vars()?)),
            other => {
                let span = other.span();
                let actual_type = other.expr_type();
                self.put_back(other);
                Err(AstError::WrongExprType(span, AstType::Literal, actual_type))
            }
        }
    }

    pub fn new(span: Span, iter: I) -> Self {
        AstIterator { remaining_span: span, iter: itertools::put_back(iter) }
    }

    pub fn expect_any(&mut self) -> Result<Ast, AstError> {
        self.next().ok_or_else(|| AstError::TooFewElements(self.remaining_span.point_span()))
    }

    pub fn expect_simplexpr(&mut self) -> Result<(Span, SimplExpr), AstError> {
        let expr_type = AstType::SimplExpr;
        match self.expect_any()? {
            Ast::SimplExpr(span, expr) => Ok((span, expr)),
            Ast::Symbol(span, var) => Ok((span, SimplExpr::VarRef(span, VarName(var)))),
            other => {
                let span = other.span();
                let actual_type = other.expr_type();
                self.put_back(other);
                Err(AstError::WrongExprType(span, expr_type, actual_type))
            }
        }
    }

    pub fn expect_done(&mut self) -> Result<(), AstError> {
        if let Some(next) = self.next() {
            self.put_back(next);
            Err(AstError::NoMoreElementsExpected(self.remaining_span))
        } else {
            Ok(())
        }
    }

    pub fn expect_key_values(&mut self) -> Result<Attributes, AstError> {
        parse_key_values(self, true)
    }

    pub fn put_back(&mut self, ast: Ast) -> Option<Ast> {
        self.remaining_span.0 = ast.span().0;
        self.iter.put_back(ast)
    }
}

impl<I: Iterator<Item = Ast>> Iterator for AstIterator<I> {
    type Item = Ast;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|x| {
            self.remaining_span.0 = x.span().1;
            x
        })
    }
}

/// Parse consecutive `:keyword value` pairs from an expression iterator into an [Attributes].
fn parse_key_values(
    iter: &mut AstIterator<impl Iterator<Item = Ast>>,
    fail_on_dangling_kw: bool,
) -> Result<Attributes, AstError> {
    let mut data = HashMap::new();
    let mut attrs_span = iter.remaining_span.point_span();
    loop {
        match iter.next() {
            Some(Ast::Keyword(key_span, kw)) => match iter.next() {
                Some(value) => {
                    attrs_span.1 = iter.remaining_span.0;
                    let attr_value = AttrEntry { key_span, value };
                    data.insert(AttrName(kw), attr_value);
                }
                None => {
                    if fail_on_dangling_kw {
                        return Err(AstError::DanglingKeyword(key_span, kw.into()));
                    } else {
                        iter.iter.put_back(Ast::Keyword(key_span, kw));
                        attrs_span.1 = iter.remaining_span.0;
                        return Ok(Attributes::new(attrs_span, data));
                    }
                }
            },
            next => {
                if let Some(expr) = next {
                    iter.iter.put_back(expr);
                }
                attrs_span.1 = iter.remaining_span.0;
                return Ok(Attributes::new(attrs_span, data));
            }
        }
    }
}
