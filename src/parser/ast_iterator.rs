use itertools::Itertools;
use simplexpr::{ast::SimplExpr, dynval::DynVal};
use std::collections::HashMap;

use std::fmt::Display;

use super::{
    ast::{Ast, AstType, Span},
    from_ast::FromAst,
};
use crate::{
    config::attributes::{AttrEntry, Attributes},
    error::{AstError, AstResult, OptionAstErrorExt},
    value::AttrName,
};

pub struct AstIterator<I: Iterator<Item = Ast>> {
    remaining_span: Span,
    iter: itertools::PutBack<I>,
}

macro_rules! return_or_put_back {
    ($name:ident, $expr_type:expr, $t:ty = $p:pat => $ret:expr) => {
        pub fn $name(&mut self) -> AstResult<$t> {
            let expr_type = $expr_type;
            match self.expect_any()? {
                $p => {
                    let (span, value) = $ret;
                    self.remaining_span.1 = span.1;
                    Ok((span, value))
                }
                other => {
                    let span = other.span();
                    let actual_type = other.expr_type();
                    self.iter.put_back(other);
                    Err(AstError::WrongExprType(span, expr_type, actual_type))
                }
            }
        }
    };
}

impl<I: Iterator<Item = Ast>> AstIterator<I> {
    return_or_put_back!(expect_symbol, AstType::Symbol, (Span, String) = Ast::Symbol(span, x) => (span, x));

    return_or_put_back!(expect_literal, AstType::Literal, (Span, DynVal) = Ast::Literal(span, x) => (span, x));

    return_or_put_back!(expect_list, AstType::List, (Span, Vec<Ast>) = Ast::List(span, x) => (span, x));

    return_or_put_back!(expect_array, AstType::Array, (Span, Vec<Ast>) = Ast::Array(span, x) => (span, x));

    pub fn new(span: Span, iter: I) -> Self {
        AstIterator { remaining_span: span, iter: itertools::put_back(iter) }
    }

    pub fn expect_any<T: FromAst>(&mut self) -> AstResult<T> {
        self.iter.next().or_missing(self.remaining_span.with_length(0)).and_then(T::from_ast)
    }

    pub fn expect_key_values(&mut self) -> AstResult<Attributes> {
        parse_key_values(self)
    }
}

impl<I: Iterator<Item = Ast>> Iterator for AstIterator<I> {
    type Item = Ast;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Parse consecutive `:keyword value` pairs from an expression iterator into an [Attributes].
fn parse_key_values(iter: &mut AstIterator<impl Iterator<Item = Ast>>) -> AstResult<Attributes> {
    let mut data = HashMap::new();
    let mut attrs_span = Span(iter.remaining_span.0, iter.remaining_span.0, iter.remaining_span.1);
    loop {
        match iter.next() {
            Some(Ast::Keyword(key_span, kw)) => match iter.next() {
                Some(value) => {
                    attrs_span.1 = iter.remaining_span.0;
                    let attr_value = AttrEntry { key_span, value };
                    data.insert(AttrName(kw), attr_value);
                }
                None => {
                    iter.iter.put_back(Ast::Keyword(key_span, kw));
                    attrs_span.1 = iter.remaining_span.0;
                    return Ok(Attributes::new(attrs_span, data));
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
