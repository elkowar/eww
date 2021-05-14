#![allow(unused_imports)]

use lalrpop_util::lalrpop_mod;

mod lexer;

lalrpop_mod!(pub calc);

#[derive(Debug)]
pub enum Expr {
    List(Vec<Expr>),
    Symbol(String),
    Number(i32),
}

fn main() {}

#[allow(unused_macros)]
macro_rules! test_p {
    ($e:expr) => {
        let e = $e;
        let lex = crate::lexer::Lexer::new(e);
        let p = calc::ExprParser::new();
        match p.parse(lex) {
            Ok(res) => println!("{:?}", res),
            Err(e) => eprintln!("{:?}", e),
        }
    };
}

#[test]
fn calc() {
    //assert!(calc::ExprParser::new().parse("(1 2 3)").is_ok());

    test_p!("1");
    test_p!("(12)");
    test_p!("(1 2)");

    println!("\n\n\n\n\n\n");

    panic!()
}
