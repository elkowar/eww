use itertools::Itertools;
use simplexpr::{ast::SimplExpr, dynval::DynVal};
use std::collections::HashMap;

use std::fmt::Display;

use super::from_ast::FromAst;
use crate::{
    config::attributes::{AttrEntry, Attributes},
    error::{AstError, AstResult, OptionAstErrorExt},
    value::AttrName,
};

#[derive(Eq, PartialEq, Clone, Copy, serde::Serialize)]
pub struct Span(pub usize, pub usize, pub usize);

impl Span {
    /// Get the span that includes this and the other span completely.
    /// Will panic if the spans are from different file_ids.
    pub fn to(mut self, other: Span) -> Self {
        assert!(other.2 == self.2);
        self.1 = other.1;
        self
    }

    pub fn ending_at(mut self, end: usize) -> Self {
        self.1 = end;
        self
    }

    pub fn with_length(mut self, end: usize) -> Self {
        self.1 = self.0;
        self
    }
}

impl Into<simplexpr::Span> for Span {
    fn into(self) -> simplexpr::Span {
        simplexpr::Span(self.0, self.1, self.2)
    }
}
impl From<simplexpr::Span> for Span {
    fn from(x: simplexpr::Span) -> Span {
        Span(x.0, x.1, x.2)
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.0, self.1)
    }
}

impl std::fmt::Debug for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.0, self.1)
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum AstType {
    List,
    Array,
    Keyword,
    Symbol,
    Literal,
    SimplExpr,
    Comment,
    // A value that could be used as a [SimplExpr]
    IntoPrimitive,
}

impl Display for AstType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(PartialEq, Eq, Clone, serde::Serialize)]
pub enum Ast {
    List(Span, Vec<Ast>),
    Array(Span, Vec<Ast>),
    Keyword(Span, String),
    Symbol(Span, String),
    Literal(Span, DynVal),
    SimplExpr(Span, SimplExpr),
    Comment(Span),
}

macro_rules! as_func {
    ($exprtype:expr, $name:ident $nameref:ident < $t:ty > = $p:pat => $value:expr) => {
        pub fn $name(self) -> Result<$t, AstError> {
            match self {
                $p => Ok($value),
                x => Err(AstError::WrongExprType(Some(x.span()), $exprtype, x.expr_type())),
            }
        }

        pub fn $nameref(&self) -> Result<&$t, AstError> {
            match self {
                $p => Ok($value),
                x => Err(AstError::WrongExprType(Some(x.span()), $exprtype, x.expr_type())),
            }
        }
    };
}

impl Ast {
    as_func!(AstType::Literal, as_literal as_literal_ref<DynVal> = Ast::Literal(_, x) => x);

    as_func!(AstType::Symbol, as_symbol as_symbol_ref<String> = Ast::Symbol(_, x) => x);

    as_func!(AstType::Keyword, as_keyword as_keyword_ref<String> = Ast::Keyword(_, x) => x);

    as_func!(AstType::List, as_list as_list_ref<Vec<Ast>> = Ast::List(_, x) => x);

    pub fn expr_type(&self) -> AstType {
        match self {
            Ast::List(..) => AstType::List,
            Ast::Array(..) => AstType::Array,
            Ast::Keyword(..) => AstType::Keyword,
            Ast::Symbol(..) => AstType::Symbol,
            Ast::Literal(..) => AstType::Literal,
            Ast::SimplExpr(..) => AstType::SimplExpr,
            Ast::Comment(_) => AstType::Comment,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Ast::List(span, _) => *span,
            Ast::Array(span, _) => *span,
            Ast::Keyword(span, _) => *span,
            Ast::Symbol(span, _) => *span,
            Ast::Literal(span, _) => *span,
            Ast::SimplExpr(span, _) => *span,
            Ast::Comment(span) => *span,
        }
    }

    pub fn as_simplexpr(self) -> AstResult<SimplExpr> {
        match self {
            // do I do this?
            // Ast::Array(_, _) => todo!(),
            // Ast::Symbol(_, _) => todo!(),
            Ast::Literal(span, x) => Ok(SimplExpr::Literal(span.into(), x)),
            Ast::SimplExpr(span, x) => Ok(x),
            _ => Err(AstError::WrongExprType(Some(self.span()), AstType::IntoPrimitive, self.expr_type())),
        }
    }

    pub fn try_ast_iter(self) -> AstResult<AstIterator<impl Iterator<Item = Ast>>> {
        let span = self.span();
        let list = self.as_list()?;
        Ok(AstIterator::new(span, list.into_iter()))
    }
}

impl std::fmt::Display for Ast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Ast::*;
        match self {
            List(_, x) => write!(f, "({})", x.iter().map(|e| format!("{}", e)).join(" ")),
            Array(_, x) => write!(f, "({})", x.iter().map(|e| format!("{}", e)).join(" ")),
            Keyword(_, x) => write!(f, ":{}", x),
            Symbol(_, x) => write!(f, "{}", x),
            Literal(_, x) => write!(f, "\"{}\"", x),
            SimplExpr(_, x) => write!(f, "{{{}}}", x),
            Comment(_) => write!(f, ""),
        }
    }
}
impl std::fmt::Debug for Ast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Ast::*;
        write!(f, "{}", self)
        // match self {
        // List(span, x) => f.debug_tuple(&format!("List<{}>", span)).field(x).finish(),
        // Array(span, x) => f.debug_tuple(&format!("Array<{}>", span)).field(x).finish(),
        // Keyword(span, x) => write!(f, "Number<{}>({})", span, x),
        // Symbol(span, x) => write!(f, "Symbol<{}>({})", span, x),
        // Value(span, x) => write!(f, "Value<{}>({})", span, x),
        // SimplExpr(span, x) => write!(f, "SimplExpr<{}>({})", span, x),
        // Comment(span) => write!(f, "Comment<{}>", span),
        //}
    }
}

pub struct AstIterator<I: Iterator<Item = Ast>> {
    remaining_span: Span,
    iter: itertools::PutBack<I>,
}

macro_rules! return_or_put_back {
    ($name:ident, $expr_type:expr, $t:ty = $p:pat => $ret:expr) => {
        pub fn $name(&mut self) -> AstResult<$t> {
            let expr_type = $expr_type;
            match self.next() {
                Some($p) => {
                    let (span, value) = $ret;
                    self.remaining_span.1 = span.1;
                    Ok((span, value))
                }
                Some(other) => {
                    let span = other.span();
                    let actual_type = other.expr_type();
                    self.iter.put_back(other);
                    Err(AstError::WrongExprType(Some(span), expr_type, actual_type))
                }
                None => Err(AstError::MissingNode(None)),
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
        self.iter.next().or_missing().and_then(T::from_ast)
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
                    let attr_value = AttrEntry { key_span, value: value.as_simplexpr()? };
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
