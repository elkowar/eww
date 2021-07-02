#![allow(unused_imports)]
#![allow(unused)]
#![feature(try_blocks)]

mod config;
mod error;
use error::AstError;

use std::{fmt::Display, ops::Deref};

use itertools::Itertools;

use lalrpop_util::lalrpop_mod;

// mod lexer;

lalrpop_mod!(pub parser);

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

impl From<Expr> for ExprType {
    fn from(x: Expr) -> Self {
        match x {
            Expr::List(..) => ExprType::List,
            Expr::Table(..) => ExprType::Table,
            Expr::Keyword(..) => ExprType::Keyword,
            Expr::Symbol(..) => ExprType::Symbol,
            Expr::Str(..) => ExprType::Str,
            Expr::Number(..) => ExprType::Number,
            Expr::Comment(_) => ExprType::Number,
        }
    }
}

macro_rules! as_func {
    ($exprtype:expr, $name:ident < $t:ty > = $p:pat => $value:expr) => {
        fn $name(self) -> Result<$t, AstError> {
            match self {
                $p => Ok($value),
                x => Err(AstError::WrongExprType(Some(x.span()), $exprtype, x)),
            }
        }
    };
}

impl Expr {
    as_func!(ExprType::Str, as_str<String> = Expr::Str(_, x) => x);

    as_func!(ExprType::Symbol, as_symbol<String> = Expr::Symbol(_, x) => x);

    as_func!(ExprType::List, as_list<Vec<Expr>> = Expr::List(_, x) => x);

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

fn main() {}

#[allow(unused_macros)]
macro_rules! test_p {
    ($e:expr) => {
        let p = parser::ExprParser::new();
        insta::assert_debug_snapshot!(p.parse($e))
    };
}

#[test]
fn test() {
    test_p!("1");
    test_p!("(12)");
    test_p!("(1 2)");
    test_p!("(1 :foo 1)");
    test_p!("(:foo 1)");
    test_p!("(:foo->: 1)");
    test_p!("(foo 1)");
    test_p!("(lol😄 1)");

    test_p!(r#"(test "hi")"#);
    test_p!(r#"(test "h\"i")"#);
    test_p!(r#"(test " hi ")"#);

    test_p!("(+ (1 2 (* 2 5)))");

    test_p!(r#"{:key value 12 "hi" (test) (1 2 3)}"#);

    test_p!(r#"; test"#);
    test_p!(
        r#"(f arg ; test
     arg2)"#
    );
}
