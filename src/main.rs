#![allow(unused_imports)]
#![allow(unused)]

mod config;
mod error;
use error::AstError;

use std::ops::Deref;

use itertools::Itertools;

use lalrpop_util::lalrpop_mod;

//mod lexer;

lalrpop_mod!(pub parser);

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct Span(pub usize, pub usize);

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<{}..{}>", self.0, self.1)
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
    Comment,
}

macro_rules! as_func {
    ($name:ident<$t:ty> = $p:pat => $value:expr) => {
        fn $name(self) -> Result<$t, AstError> {
            match self {
                $p => Ok($value),
                x => Err(AstError::WrongExprType(x)),
            }
        }
    };
}

impl Expr {
    as_func!(as_str<String> = Expr::Str(_, x) => x);
    as_func!(as_symbol<String> = Expr::Symbol(_, x) => x);

    fn is_keyword(&self) -> bool {
        match self {
            Expr::Keyword(_, _) => true,
            _ => false,
        }
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Expr::*;
        match self {
            Number(_, x) => write!(f, "{}", x),
            List(_, x) => write!(f, "({})", x.iter().map(|e| format!("{}", e)).join(" ")),
            Table(_, x) => write!(
                f,
                "{{{}}}",
                x.iter().map(|(k, v)| format!("{} {}", k, v)).join(" ")
            ),
            Keyword(_, x) => write!(f, "{}", x),
            Symbol(_, x) => write!(f, "{}", x),
            Str(_, x) => write!(f, "{}", x),
            Comment => write!(f, ""),
        }
    }
}

fn main() {}

#[allow(unused_macros)]
macro_rules! test_p {
    ($e:expr) => {
        let e = $e;
        let p = parser::ExprParser::new();
        match p.parse(e) {
            Ok(res) => println!("{}\n=> {}\n", e, res),
            Err(e) => eprintln!("{}", e),
        }
    };
}

#[test]
fn test() {
    //test_p!("1");
    //test_p!("(12)");
    //test_p!("(1 2)");
    //test_p!("(1 :foo 1)");
    //test_p!("(:foo 1)");
    //test_p!("(:foo->: 1)");
    //test_p!("(foo 1)");
    //test_p!("(lol😄 1)");

    //test_p!(r#"(test "hi")"#);
    //test_p!(r#"(test "h\"i")"#);
    //test_p!(r#"(test " hi ")"#);

    //test_p!("(+ (1 2 (* 2 5)))");

    //test_p!(r#"{:key value 12 "hi" (test) (1 2 3)}"#);

    //test_p!(r#"; test"#);
    //test_p!(
    //r#"(f arg ; test
    //arg2)"#
    //);

    //println!("\n\n\n\n\n\n");
}
