use eww_config::{config::*, expr::*};

fn main() {
    let mut files = codespan_reporting::files::SimpleFiles::new();

    let input = "(12 :bar 22 (foo) (baz))";

    let file_id = files.add("foo.eww", input);
    let ast = eww_config::parse_string(file_id, input);
    match ast.and_then(Element::<Expr, Expr>::from_expr) {
        Ok(ast) => {
            println!("{:?}", ast);
        }
        Err(err) => {
            let diag = err.pretty_diagnostic(&files);
            use codespan_reporting::term;
            let mut writer = term::termcolor::StandardStream::stderr(term::termcolor::ColorChoice::Always);
            term::emit(&mut writer, &term::Config::default(), &files, &diag).unwrap();
        }
    }
}
