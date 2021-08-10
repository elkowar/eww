// use eww_config::{
// format_diagnostic::ToDiagnostic,
// parser::{ast::*, from_ast::FromAst},
//};

fn main() {
    println!("hi");
    // let mut files = codespan_reporting::files::SimpleFiles::new();

    // let input = r#"
    //(heyho ; :foo { "foo \" } bar " }
    //; :baz {(foo == bar ? 12.2 : 12)}
    //(foo)
    //(defwidget foo [something bla] "foo")
    //(baz))"#;

    // let file_id = files.add("foo.eww", input);
    // let ast = eww_config::parser::parse_string(file_id, input);
    // match ast.and_then(eww_config::parser::from_ast::Element::<Ast, Ast>::from_ast) {
    // Ok(ast) => {
    // println!("{:?}", ast);
    //}
    // Err(err) => {
    // dbg!(&err);
    // let diag = err.to_diagnostic();
    // use codespan_reporting::term;
    // let config = term::Config::default();
    // let mut writer = term::termcolor::StandardStream::stderr(term::termcolor::ColorChoice::Always);
    // term::emit(&mut writer, &config, &files, &diag).unwrap();
    //}
}
