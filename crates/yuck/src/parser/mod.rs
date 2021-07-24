use eww_shared_util::Span;
use lalrpop_util::lalrpop_mod;

use super::error::{AstError, AstResult};
use ast::Ast;

use std::{fmt::Display, ops::Deref};

use itertools::Itertools;

pub mod ast;
pub mod ast_iterator;
pub mod from_ast;
pub(crate) mod lexer;
pub(crate) mod parse_error;

lalrpop_mod!(
    #[allow(clippy::all)]
    pub parser,
    "/parser/parser.rs"
);

pub fn parse_string(file_id: usize, s: &str) -> AstResult<Ast> {
    let lexer = lexer::Lexer::new(file_id, s.to_string());
    let parser = parser::AstParser::new();
    parser.parse(file_id, lexer).map_err(|e| AstError::from_parse_error(file_id, e))
}

/// Parse multiple toplevel nodes into a list of [Ast]
pub fn parse_toplevel(file_id: usize, s: String) -> AstResult<(Span, Vec<Ast>)> {
    let lexer = lexer::Lexer::new(file_id, s);
    let parser = parser::ToplevelParser::new();
    parser.parse(file_id, lexer).map_err(|e| AstError::from_parse_error(file_id, e))
}

/// get a single ast node from a list of asts, returning an Err if the length is not exactly 1.
pub fn require_single_toplevel(span: Span, mut asts: Vec<Ast>) -> AstResult<Ast> {
    match asts.len() {
        0 => Err(AstError::MissingNode(span)),
        1 => Ok(asts.remove(0)),
        _ => Err(AstError::TooManyNodes(asts.get(1).unwrap().span().to(asts.last().unwrap().span()), 1)),
    }
}

macro_rules! test_parser {
    ($($text:literal),*) => {{
        let p = parser::AstParser::new();
        use lexer::Lexer;

        ::insta::with_settings!({sort_maps => true}, {
            $(
                ::insta::assert_debug_snapshot!(p.parse(0, Lexer::new(0, $text.to_string())));
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
        r#"foo ; test"#,
        r#"(f arg ; test
        arg2)"#,
        "\"h\\\"i\""
    );
}
