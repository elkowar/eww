use itertools::Itertools;
use std::collections::HashMap;

use crate::{config::FromAst, error::*};
use std::fmt::Display;

#[derive(Eq, PartialEq, Clone, Copy)]
pub struct Span(pub usize, pub usize, pub usize);

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
    Keyword,
    Symbol,
    Value,
    Comment,
}

impl Display for AstType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(PartialEq, Eq, Clone)]
pub enum Ast {
    List(Span, Vec<Ast>),
    // ArgList(Span, Vec<Ast>),
    Keyword(Span, String),
    Symbol(Span, String),
    Value(Span, String),
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
    as_func!(AstType::Value, as_value as_value_ref<String> = Ast::Value(_, x) => x);

    as_func!(AstType::Symbol, as_symbol as_symbol_ref<String> = Ast::Symbol(_, x) => x);

    as_func!(AstType::Keyword, as_keyword as_keyword_ref<String> = Ast::Keyword(_, x) => x);

    as_func!(AstType::List, as_list as_list_ref<Vec<Ast>> = Ast::List(_, x) => x);

    pub fn expr_type(&self) -> AstType {
        match self {
            Ast::List(..) => AstType::List,
            Ast::Keyword(..) => AstType::Keyword,
            Ast::Symbol(..) => AstType::Symbol,
            Ast::Value(..) => AstType::Value,
            Ast::Comment(_) => AstType::Comment,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Ast::List(span, _) => *span,
            Ast::Keyword(span, _) => *span,
            Ast::Symbol(span, _) => *span,
            Ast::Value(span, _) => *span,
            Ast::Comment(span) => *span,
        }
    }

    pub fn first_list_elem(&self) -> Option<&Ast> {
        match self {
            Ast::List(_, list) => list.first(),
            _ => None,
        }
    }
}

impl std::fmt::Display for Ast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Ast::*;
        match self {
            List(_, x) => write!(f, "({})", x.iter().map(|e| format!("{}", e)).join(" ")),
            Keyword(_, x) => write!(f, "{}", x),
            Symbol(_, x) => write!(f, "{}", x),
            Value(_, x) => write!(f, "{}", x),
            Comment(_) => write!(f, ""),
        }
    }
}
impl std::fmt::Debug for Ast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Ast::*;
        match self {
            List(span, x) => f.debug_tuple(&format!("List<{}>", span)).field(x).finish(),
            Keyword(span, x) => write!(f, "Number<{}>({})", span, x),
            Symbol(span, x) => write!(f, "Symbol<{}>({})", span, x),
            Value(span, x) => write!(f, "Value<{}>({})", span, x),
            Comment(span) => write!(f, "Comment<{}>", span),
        }
    }
}

pub struct AstIterator<I: Iterator<Item = Ast>> {
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

impl<I: Iterator<Item = Ast>> AstIterator<I> {
    return_or_put_back!(expect_symbol, AstType::Symbol, (Span, String) = Ast::Symbol(span, x) => (span, x));

    return_or_put_back!(expect_string, AstType::Value, (Span, String) = Ast::Value(span, x) => (span, x));

    return_or_put_back!(expect_list, AstType::List, (Span, Vec<Ast>) = Ast::List(span, x) => (span, x));

    pub fn new(iter: I) -> Self {
        AstIterator { iter: itertools::put_back(iter) }
    }

    pub fn expect_key_values<T: FromAst>(&mut self) -> AstResult<HashMap<String, T>> {
        parse_key_values(&mut self.iter)
    }
}

impl<I: Iterator<Item = Ast>> Iterator for AstIterator<I> {
    type Item = Ast;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Parse consecutive `:keyword value` pairs from an expression iterator into a HashMap. Transforms the keys using the FromExpr trait.
fn parse_key_values<T: FromAst, I: Iterator<Item = Ast>>(iter: &mut itertools::PutBack<I>) -> AstResult<HashMap<String, T>> {
    let mut data = HashMap::new();
    loop {
        match iter.next() {
            Some(Ast::Keyword(span, kw)) => match iter.next() {
                Some(value) => {
                    data.insert(kw, T::from_ast(value)?);
                }
                None => {
                    iter.put_back(Ast::Keyword(span, kw));
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
