use itertools::Itertools;
use simplexpr::{ast::SimplExpr, dynval::DynVal};
use std::collections::HashMap;

use std::fmt::Display;

use super::{ast_iterator::AstIterator, from_ast::FromAst};
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
    /// A value that could be used as a [SimplExpr]
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
                x => Err(AstError::WrongExprType(x.span(), $exprtype, x.expr_type())),
            }
        }

        pub fn $nameref(&self) -> Result<&$t, AstError> {
            match self {
                $p => Ok($value),
                x => Err(AstError::WrongExprType(x.span(), $exprtype, x.expr_type())),
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
            _ => Err(AstError::WrongExprType(self.span(), AstType::IntoPrimitive, self.expr_type())),
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
