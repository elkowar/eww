use eww_config::{
    config::{widget_definition::WidgetDefinition, widget_use::WidgetUse, *},
    error::AstError,
    format_diagnostic::ToDiagnostic,
    parser::from_ast::FromAst,
};

fn main() {
    let mut files = codespan_reporting::files::SimpleFiles::new();

    let input_use = r#"
        (foo :something 12
             :bla "bruh"
          "some text")
    "#;
    let input_def = r#"
        (defwidget foo [something bla] "foo")
    "#;

    let file_id_use = files.add("use.eww", input_use);
    let file_id_def = files.add("def.eww", input_def);
    let parsed_use = WidgetUse::from_ast(eww_config::parser::parse_string(file_id_use, input_use).unwrap()).unwrap();
    let parsed_def = WidgetDefinition::from_ast(eww_config::parser::parse_string(file_id_def, input_def).unwrap()).unwrap();
    let defs = maplit::hashmap! {
        "foo".to_string() => parsed_def,
    };
    match validate::validate(&defs, &parsed_use) {
        Ok(ast) => {
            println!("{:?}", ast);
        }
        Err(err) => {
            let err = AstError::ValidationError(err);
            let diag = err.to_diagnostic();
            use codespan_reporting::term;
            let config = term::Config::default();
            let mut writer = term::termcolor::StandardStream::stderr(term::termcolor::ColorChoice::Always);
            term::emit(&mut writer, &config, &files, &diag).unwrap();
        }
    }
}
