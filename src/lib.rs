#![feature(box_patterns)]
#![feature(box_syntax)]
#![feature(try_blocks)]
pub mod ast;
pub mod dynval;
pub mod error;
pub mod eval;
pub mod parser;
use ast::SimplExpr;
use lalrpop_util::lalrpop_mod;

lalrpop_mod!(pub simplexpr_parser);

pub fn parse_string(s: &str) -> Result<SimplExpr, error::Error> {
    parser::parse_string(s)
}
pub use ast::Span;
