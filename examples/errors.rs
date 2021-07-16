use std::collections::HashMap;

use simplexpr::dynval::DynVal;

fn main() {
    let mut files = codespan_reporting::files::SimpleFiles::new();

    let input = "12 + foo * 2 < 2 ? bar == true : false";

    let _ = files.add("foo.eww", input);
    let ast = simplexpr::parser::parse_string(input);

    let mut vars = HashMap::new();
    vars.insert("foo".to_string(), "2".into());

    match ast.and_then(|x| x.eval(&vars).map_err(|e| e.into())) {
        Ok(ast) => {
            println!("{:?}", ast);
        }
        Err(err) => {
            let diag = err.pretty_diagnostic();
            use codespan_reporting::term;
            let mut writer = term::termcolor::StandardStream::stderr(term::termcolor::ColorChoice::Always);
            term::emit(&mut writer, &term::Config::default(), &files, &diag).unwrap();
        }
    }
}
