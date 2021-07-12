pub mod ast;
use lalrpop_util::lalrpop_mod;

lalrpop_mod!(pub parser);

macro_rules! test_parser {
    ($($text:literal),*) => {{
        let p = crate::parser::ExprParser::new();
        //use crate::lexer::Lexer;

        ::insta::with_settings!({sort_maps => true}, {
            $(
                ::insta::assert_debug_snapshot!(p.parse($text));
            )*
        });
    }}
}

#[cfg(test)]
mod tests {
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
            "! false || ! true"
        );
    }
}
