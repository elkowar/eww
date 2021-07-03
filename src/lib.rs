#![allow(unused_imports)]
#![allow(unused)]
#![feature(try_blocks)]

pub mod config;
pub mod error;
pub mod expr;
use error::AstError;

use std::{fmt::Display, ops::Deref};

use itertools::Itertools;

use lalrpop_util::lalrpop_mod;

lalrpop_mod!(pub parser);

macro_rules! test_parser {
    ($p:expr, $($text:literal),*) => {{
        $(insta::assert_debug_snapshot!($p.parse(0, $text));)*
    }}
}

#[test]
fn test() {
    let p = parser::ExprParser::new();
    test_parser!(
        p,
        "1",
        "(12)",
        "(1 2)",
        "(1 :foo 1)",
        "(:foo 1)",
        "(:foo->: 1)",
        "(foo 1)",
        "(lol😄 1)",
        r#"(test "hi")"#,
        r#"(test "h\"i")"#,
        r#"(test " hi ")"#,
        "(+ (1 2 (* 2 5)))",
        r#"{:key value 12 "hi" (test) (1 2 3)}"#,
        r#"; test"#,
        r#"(f arg ; test
        arg2)"#,
        "\"h\\\"i\""
    );
}
