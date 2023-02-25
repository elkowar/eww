use eww_shared_util::{Span, Spanned};
use lalrpop_util::lalrpop_mod;

use crate::gen_diagnostic;

use super::error::{DiagError, DiagResult};
use ast::Ast;

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

pub fn parse_string(file_id: usize, s: &str) -> DiagResult<Ast> {
    let lexer = lexer::Lexer::new(file_id, s.to_string());
    let parser = parser::AstParser::new();
    parser.parse(file_id, lexer).map_err(|e| DiagError::from_parse_error(file_id, e))
}

/// Parse multiple toplevel nodes into a list of [Ast]
pub fn parse_toplevel(file_id: usize, s: String) -> DiagResult<(Span, Vec<Ast>)> {
    let lexer = lexer::Lexer::new(file_id, s);
    let parser = parser::ToplevelParser::new();
    parser.parse(file_id, lexer).map_err(|e| DiagError::from_parse_error(file_id, e))
}

/// get a single ast node from a list of asts, returning an Err if the length is not exactly 1.
pub fn require_single_toplevel(span: Span, mut asts: Vec<Ast>) -> DiagResult<Ast> {
    match asts.len() {
        1 => Ok(asts.remove(0)),
        0 => Err(DiagError(gen_diagnostic! {
            msg = "Expected exactly one element, but got none",
            label = span
        })),
        _n => Err(DiagError(gen_diagnostic! {
            msg = "Expected exactly one element, but but got {n}",
            label = asts.get(1).unwrap().span().to(asts.last().unwrap().span()) => "these elements must not be here",
            note = "Consider wrapping the elements in some container element",
        })),
    }
}

#[cfg(test)]
mod test {
    use super::*;
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
}
