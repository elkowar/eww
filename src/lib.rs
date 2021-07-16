#![feature(box_patterns)]
#![feature(box_syntax)]
#![feature(try_blocks)]
pub mod ast;
pub mod dynval;
pub mod error;
pub mod eval;
pub mod parser;
use lalrpop_util::lalrpop_mod;

lalrpop_mod!(pub simplexpr_parser);
