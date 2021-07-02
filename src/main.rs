#![allow(unused_imports)]
#![allow(unused)]
#![feature(try_blocks)]

mod config;
mod error;
mod expr;
use error::AstError;

use std::{fmt::Display, ops::Deref};

use itertools::Itertools;

use lalrpop_util::lalrpop_mod;

// mod lexer;

lalrpop_mod!(pub parser);

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
