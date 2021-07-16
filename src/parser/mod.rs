pub mod lalrpop_helpers;
pub mod lexer;

use crate::{
    ast::SimplExpr,
    error::{Error, Result},
};

pub fn parse_string(s: &str) -> Result<SimplExpr> {
    let lexer = lexer::Lexer::new(s);
    let parser = crate::simplexpr_parser::ExprParser::new();
    Ok(parser.parse(lexer).map_err(|e| Error::from_parse_error(e))?)
}

#[cfg(test)]
mod tests {
    macro_rules! test_parser {
        ($($text:literal),* $(,)?) => {{
            let p = crate::simplexpr_parser::ExprParser::new();
            use crate::parser::lexer::Lexer;
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
