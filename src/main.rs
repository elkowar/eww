#![allow(unused_imports)]
#![allow(unused)]

mod config;

use std::ops::Deref;

use itertools::Itertools;

use lalrpop_util::lalrpop_mod;

//mod lexer;

lalrpop_mod!(pub calc);

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct Sp<T>(pub usize, pub T, pub usize);

impl<T: std::fmt::Display> std::fmt::Display for Sp<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<{}- {} -{}>", self.0, self.1, self.2)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WrongExprType;

#[derive(Debug)]
pub enum Expr {
    List(Vec<Sp<Expr>>),
    Table(Vec<(Sp<Expr>, Sp<Expr>)>),
    Keyword(String),
    Symbol(String),
    Str(String),
    Number(i32),
    Comment,
}

impl Expr {
    fn str(self) -> Result<String, WrongExprType> {
        use Expr::*;
        match self {
            Str(x) => Ok(x),
            _ => Err(WrongExprType),
        }
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Expr::*;
        match self {
            Number(x) => write!(f, "{}", x),
            List(x) => write!(f, "({})", x.iter().map(|e| format!("{}", e)).join(" ")),
            Table(x) => write!(
                f,
                "{{{}}}",
                x.iter().map(|(k, v)| format!("{} {}", k, v)).join(" ")
            ),
            Keyword(x) => write!(f, "{}", x),
            Symbol(x) => write!(f, "{}", x),
            Str(x) => write!(f, "{}", x),
            Comment => write!(f, ""),
        }
    }
}

fn main() {}

#[allow(unused_macros)]
macro_rules! test_p {
    ($e:expr) => {
        let e = $e;
        let p = calc::ExprParser::new();
        match p.parse(e) {
            Ok(res) => println!("{}\n=> {}\n", e, res),
            Err(e) => eprintln!("{}", e),
        }
    };
}

#[test]
fn calc() {
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

    println!("\n\n\n\n\n\n");

    panic!()
}
