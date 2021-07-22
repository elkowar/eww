use crate::{
    config::config::Config,
    parser::{self, ast::Ast, from_ast::FromAst, lexer::Lexer},
};

#[test]
fn test_config() {
    let input = r#"
        (defwidget foo [arg]
            "heyho")
        (defwidget bar [arg arg2]
            "bla")
        (defvar some_var "bla")
        (defpollvar stuff :interval "12s" "date")
        (deftailvar stuff "tail -f stuff")
        (defwindow some-window
                   :stacking "fg"
                   :monitor 12
                   :resizable true
                   :geometry (geometry :width "12%" :height "20px")
                   :reserve (struts :side "left" :distance "30px")
            (foo :arg "bla"))
    "#;

    let lexer = Lexer::new(0, input.to_string());
    let p = parser::parser::ToplevelParser::new();
    let (span, parse_result) = p.parse(0, lexer).unwrap();
    // TODO implement another YuckFiles thing to test here again
    // let config = Config::from_ast(Ast::List(span, parse_result));
    // insta::with_settings!({sort_maps => true}, {
    // insta::assert_ron_snapshot!(config.unwrap());
    //});
}
