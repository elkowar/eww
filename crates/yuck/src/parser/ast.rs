use itertools::Itertools;
use simplexpr::ast::SimplExpr;

use eww_shared_util::{Span, Spanned, VarName};
use std::fmt::Display;

use super::ast_iterator::AstIterator;
use crate::ast_error::AstError;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum AstType {
    List,
    Array,
    Keyword,
    Symbol,
    // TODO this does no longer correspond to an actual literal ast type as that's replaced with SimplExpr
    Literal,
    SimplExpr,
    Comment,
    /// A value that could be used as a [SimplExpr]
    IntoPrimitive,
}

impl Display for AstType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AstType::IntoPrimitive => write!(f, "primitive"),
            _ => write!(f, "{:?}", self),
        }
    }
}

#[derive(PartialEq, Eq, Clone, serde::Serialize)]
pub enum Ast {
    /// I.e.: `(foo bar baz)`
    List(Span, Vec<Ast>),
    /// I.e.: `[foo bar baz]`
    Array(Span, Vec<Ast>),
    /// I.e.: `:foo`
    Keyword(Span, String),
    /// I.e.: `foo`
    Symbol(Span, String),
    /// I.e.: `{1 + 2}`
    SimplExpr(Span, SimplExpr),
    /// I.e.: `// foo`
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
    as_func!(AstType::Symbol, as_symbol as_symbol_ref<String> = Ast::Symbol(_, x) => x);

    as_func!(AstType::Keyword, as_keyword as_keyword_ref<String> = Ast::Keyword(_, x) => x);

    as_func!(AstType::List, as_list as_list_ref<Vec<Ast>> = Ast::List(_, x) => x);

    as_func!(AstType::Array, as_array as_array_ref<Vec<Ast>> = Ast::Array(_, x) => x);

    pub fn expr_type(&self) -> AstType {
        match self {
            Ast::List(..) => AstType::List,
            Ast::Array(..) => AstType::Array,
            Ast::Keyword(..) => AstType::Keyword,
            Ast::Symbol(..) => AstType::Symbol,
            Ast::SimplExpr(..) => AstType::SimplExpr,
            Ast::Comment(_) => AstType::Comment,
        }
    }

    pub fn as_simplexpr(&self) -> Result<SimplExpr, AstError> {
        match self {
            // TODO do I do this?
            // Ast::Array(span, elements) => todo!()
            Ast::Symbol(span, x) => Ok(SimplExpr::VarRef(*span, VarName(x.clone()))),
            Ast::SimplExpr(_span, x) => Ok(x.clone()),
            _ => Err(AstError::WrongExprType(self.span(), AstType::IntoPrimitive, self.expr_type())),
        }
    }

    pub fn try_ast_iter(self) -> Result<AstIterator<impl Iterator<Item = Ast>>, AstError> {
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
            SimplExpr(_, simplexpr::SimplExpr::Literal(value)) => write!(f, "\"{}\"", value),
            SimplExpr(_, x) => write!(f, "{{{}}}", x),
            Comment(_) => write!(f, ""),
        }
    }
}
impl std::fmt::Debug for Ast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl Spanned for Ast {
    fn span(&self) -> Span {
        match self {
            Ast::List(span, _) => *span,
            Ast::Array(span, _) => *span,
            Ast::Keyword(span, _) => *span,
            Ast::Symbol(span, _) => *span,
            Ast::SimplExpr(span, _) => *span,
            Ast::Comment(span) => *span,
        }
    }
}
