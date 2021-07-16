fn main() {
    let mut files = codespan_reporting::files::SimpleFiles::new();

    let input = "12 + \"hi\" * foo ) ? bar == baz : false";

    let _ = files.add("foo.eww", input);
    let ast = simplexpr::parse_string(input);
    match ast {
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
