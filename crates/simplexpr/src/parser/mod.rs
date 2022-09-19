pub mod lalrpop_helpers;
pub mod lexer;

use crate::{ast::SimplExpr, error::ParseError};

pub fn parse_string(byte_offset: usize, file_id: usize, s: &str) -> Result<SimplExpr, ParseError> {
    let lexer = lexer::Lexer::new(file_id, byte_offset, s);
    let parser = crate::simplexpr_parser::ExprParser::new();
    parser.parse(file_id, lexer).map_err(|e| ParseError::from_parse_error(file_id, e))
}

#[cfg(test)]
mod tests {
    macro_rules! test_parser {
        ($($text:literal),* $(,)?) => {{
            let p = crate::simplexpr_parser::ExprParser::new();
            use crate::parser::lexer::Lexer;
            ::insta::with_settings!({sort_maps => true}, {
                $(
                    ::insta::assert_debug_snapshot!(p.parse(0, Lexer::new(0, 0, $text)));
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
            r#"[1, 2, 3 + 4, "bla", [blub, blo]]"#,
            r#"{ "key": "value", 5: 1+2, true: false }"#,
            r#"{ "key": "value" }?.key?.does_not_exist"#,
        );
    }
}
