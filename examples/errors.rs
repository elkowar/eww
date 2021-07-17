use eww_config::{ast::*, config::*, format_diagnostic::ToDiagnostic};

fn main() {
    let mut files = codespan_reporting::files::SimpleFiles::new();

    let input = r#"
        (hi :bar 22 :baz {(foo == bar ? 12.K : 12)} (foo) (baz))"#;

    let file_id = files.add("foo.eww", input);
    let ast = eww_config::parse_string(file_id, input);
    match ast.and_then(Element::<Ast, Ast>::from_ast) {
        Ok(ast) => {
            println!("{:?}", ast);
        }
        Err(err) => {
            dbg!(&err);
            let diag = err.to_diagnostic(&files);
            use codespan_reporting::term;
            let config = term::Config::default();
            let mut writer = term::termcolor::StandardStream::stderr(term::termcolor::ColorChoice::Always);
            term::emit(&mut writer, &config, &files, &diag).unwrap();
        }
    }
}
