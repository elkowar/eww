#![allow(unused_imports)]
#![allow(unused)]
#![feature(try_blocks)]

pub mod config;
pub mod error;
pub mod expr;
mod lexer;
use error::{AstError, AstResult};
use expr::Expr;

use std::{fmt::Display, ops::Deref};

use itertools::Itertools;

use lalrpop_util::lalrpop_mod;

lalrpop_mod!(pub parser);

pub fn parse_string(file_id: usize, s: &str) -> AstResult<Expr> {
    let lexer = lexer::Lexer::new(s);
    let parser = parser::ExprParser::new();
    Ok(parser.parse(file_id, lexer).map_err(|e| AstError::from_parse_error(file_id, e))?)
}

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
