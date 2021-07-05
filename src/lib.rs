#![allow(unused_imports)]
#![allow(unused)]
#![feature(try_blocks)]

pub mod config;
pub mod error;
pub mod expr;
pub mod lexer;
use error::AstError;

use std::{fmt::Display, ops::Deref};

use itertools::Itertools;

use lalrpop_util::lalrpop_mod;

lalrpop_mod!(pub parser);

macro_rules! test_parser {
    ($($text:literal),*) => {{
        let p = crate::parser::ExprParser::new();
        use crate::lexer::Lexer;

        ::insta::with_settings!({sort_maps => true}, {
            $(
                ::insta::assert_debug_snapshot!(p.parse(0, Lexer::new($text)));
            )*
        });
    }}
}

#[test]
fn test() {
    test_parser!(
        "1",
        "(12)",
        "1.2",
        "-1.2",
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
        r#"; test"#,
        r#"(f arg ; test
        arg2)"#,
        "\"h\\\"i\""
    );
}
