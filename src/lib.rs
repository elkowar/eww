pub mod ast;
pub mod error;
mod lalrpop_helpers;
mod lexer;
use ast::SimplExpr;
use error::{Error, Result};
use lalrpop_util::lalrpop_mod;

lalrpop_mod!(pub parser);

pub fn parse_string(s: &str) -> Result<SimplExpr> {
    let lexer = lexer::Lexer::new(s);
    let parser = parser::ExprParser::new();
    Ok(parser.parse(lexer).map_err(|e| Error::from_parse_error(e))?)
}

#[cfg(test)]
mod tests {
    macro_rules! test_parser {
        ($($text:literal),* $(,)?) => {{
            let p = crate::parser::ExprParser::new();
            use crate::lexer::Lexer;
            ::insta::with_settings!({sort_maps => true}, {
                $(
                    ::insta::assert_debug_snapshot!(p.parse(Lexer::new($text)));
                )*
            });
        }}
    }

    #[test]
    fn test() {
        test_parser!(
            "1",
            "2 + 5",
            "2 * 5 + 1 * 1 + 3",
            "(1 + 2) * 2",
            "1 + true ? 2 : 5",
            "1 + true ? 2 : 5 + 2",
            "1 + (true ? 2 : 5) + 2",
            "foo(1, 2)",
            "! false || ! true",
            "\"foo\" + 12.4",
            "hi[\"ho\"]",
            "foo.bar.baz",
            "foo.bar[2 + 2] * asdf[foo.bar]",
        );
    }
}
