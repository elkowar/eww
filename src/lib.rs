#![allow(unused_imports)]
#![allow(unused)]
#![feature(try_blocks)]

pub mod ast;
pub mod config;
pub mod error;
pub mod format_diagnostic;
mod lexer;
mod parse_error;
pub mod value;

use ast::Ast;
use error::{AstError, AstResult};

use std::{fmt::Display, ops::Deref};

use itertools::Itertools;

use lalrpop_util::lalrpop_mod;

lalrpop_mod!(
    #[allow(clippy::all)]
    pub parser
);

pub fn parse_string(file_id: usize, s: &str) -> AstResult<Ast> {
    let lexer = lexer::Lexer::new(file_id, s);
    let parser = parser::AstParser::new();
    parser.parse(file_id, lexer).map_err(|e| AstError::from_parse_error(file_id, e))
}

macro_rules! test_parser {
    ($($text:literal),*) => {{
        let p = crate::parser::AstParser::new();
        use crate::lexer::Lexer;

        ::insta::with_settings!({sort_maps => true}, {
            $(
                ::insta::assert_debug_snapshot!(p.parse(0, Lexer::new(0, $text)));
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
