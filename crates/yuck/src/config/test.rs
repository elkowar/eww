use crate::{
    config::config::Config,
    parser::{self, ast::Ast, from_ast::FromAst, lexer::Lexer},
};

use super::file_provider::YuckFiles;

#[test]
fn test_config() {
    let input = r#"
        (defwidget bar [arg arg2]
            (foo :arg "hi"))
        (defvar some_var "bla")
        (defpoll stuff :interval "12s" "date")
        (deflisten stuff "tail -f stuff")
        (defwindow some-window
                   :stacking "fg"
                   :monitor 12
                   :resizable true
                   :geometry (geometry :width "12%" :height "20px")
                   :reserve (struts :side "left" :distance "30px")
            (bar :arg "bla"))
    "#;
    let mut files = YuckFiles::new();
    let (span, asts) = files.load_str("config.yuck".to_string(), input.to_string()).unwrap();
    let config = Config::generate(&mut files, asts);
    insta::with_settings!({sort_maps => true}, {
        insta::assert_ron_snapshot!(config.unwrap());
    });
}
