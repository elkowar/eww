use itertools::Itertools;
use std::collections::HashMap;

use crate::{config::FromExpr, error::*};
use std::fmt::Display;

#[derive(Eq, PartialEq, Clone, Copy)]
pub struct Span(pub usize, pub usize);

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.0, self.1)
    }
}

impl std::fmt::Debug for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum ExprType {
    List,
    Table,
    Keyword,
    Symbol,
    Str,
    Number,
    Comment,
}

impl Display for ExprType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Expr {
    List(Span, Vec<Expr>),
    Table(Span, Vec<(Expr, Expr)>),
    Keyword(Span, String),
    Symbol(Span, String),
    Str(Span, String),
    Number(Span, i32),
    Comment(Span),
}

macro_rules! as_func {
    ($exprtype:expr, $name:ident < $t:ty > = $p:pat => $value:expr) => {
        pub fn $name(self) -> Result<$t, AstError> {
            match self {
                $p => Ok($value),
                x => Err(AstError::WrongExprType(Some(x.span()), $exprtype, x.expr_type())),
            }
        }
    };
}

impl Expr {
    as_func!(ExprType::Str, as_str<String> = Expr::Str(_, x) => x);

    as_func!(ExprType::Symbol, as_symbol<String> = Expr::Symbol(_, x) => x);

    as_func!(ExprType::List, as_list<Vec<Expr>> = Expr::List(_, x) => x);

    pub fn expr_type(&self) -> ExprType {
        match self {
            Expr::List(..) => ExprType::List,
            Expr::Table(..) => ExprType::Table,
            Expr::Keyword(..) => ExprType::Keyword,
            Expr::Symbol(..) => ExprType::Symbol,
            Expr::Str(..) => ExprType::Str,
            Expr::Number(..) => ExprType::Number,
            Expr::Comment(_) => ExprType::Number,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Expr::List(span, _) => *span,
            Expr::Table(span, _) => *span,
            Expr::Keyword(span, _) => *span,
            Expr::Symbol(span, _) => *span,
            Expr::Str(span, _) => *span,
            Expr::Number(span, _) => *span,
            Expr::Comment(span) => *span,
        }
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Expr::*;
        match self {
            Number(_, x) => write!(f, "{}", x),
            List(_, x) => write!(f, "({})", x.iter().map(|e| format!("{}", e)).join(" ")),
            Table(_, x) => write!(f, "{{{}}}", x.iter().map(|(k, v)| format!("{} {}", k, v)).join(" ")),
            Keyword(_, x) => write!(f, "{}", x),
            Symbol(_, x) => write!(f, "{}", x),
            Str(_, x) => write!(f, "{}", x),
            Comment(_) => write!(f, ""),
        }
    }
}

pub struct ExprIterator<I: Iterator<Item = Expr>> {
    iter: itertools::PutBack<I>,
}

macro_rules! return_or_put_back {
    ($name:ident, $expr_type:expr, $t:ty = $p:pat => $ret:expr) => {
        pub fn $name(&mut self) -> AstResult<$t> {
            let expr_type = $expr_type;
            match self.next() {
                Some($p) => Ok($ret),
                Some(other) => {
                    let span = other.span();
                    let actual_type = other.expr_type();
                    self.iter.put_back(other);
                    Err(AstError::WrongExprType(Some(span), expr_type, actual_type))
                }
                None => Err(AstError::MissingNode(None, expr_type)),
            }
        }
    };
}

impl<I: Iterator<Item = Expr>> ExprIterator<I> {
    return_or_put_back!(next_symbol, ExprType::Symbol, (Span, String) = Expr::Symbol(span, x) => (span, x));

    return_or_put_back!(next_string, ExprType::Str, (Span, String) = Expr::Str(span, x) => (span, x));

    pub fn new(iter: I) -> Self {
        ExprIterator { iter: itertools::put_back(iter) }
    }

    pub fn key_values<T: FromExpr>(&mut self) -> AstResult<HashMap<String, T>> {
        parse_key_values(&mut self.iter)
    }
}

impl<I: Iterator<Item = Expr>> Iterator for ExprIterator<I> {
    type Item = Expr;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Parse consecutive `:keyword value` pairs from an expression iterator into a HashMap. Transforms the keys using the FromExpr trait.
fn parse_key_values<T: FromExpr, I: Iterator<Item = Expr>>(iter: &mut itertools::PutBack<I>) -> AstResult<HashMap<String, T>> {
    let mut data = HashMap::new();
    loop {
        match iter.next() {
            Some(Expr::Keyword(span, kw)) => match iter.next() {
                Some(value) => {
                    data.insert(kw, T::from_expr(value)?);
                }
                None => {
                    iter.put_back(Expr::Keyword(span, kw));
                    return Ok(data);
                }
            },
            Some(expr) => {
                iter.put_back(expr);
                return Ok(data);
            }
            None => return Ok(data),
        }
    }
}
