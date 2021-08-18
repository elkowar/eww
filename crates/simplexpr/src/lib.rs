#![feature(box_patterns)]
#![feature(format_args_capture)]
#![feature(pattern)]
#![feature(box_syntax)]
#![feature(try_blocks)]
#![feature(unwrap_infallible)]
#![feature(never_type)]

pub mod ast;
pub mod dynval;
pub mod error;
pub mod eval;
pub mod parser;

pub use ast::SimplExpr;

use lalrpop_util::lalrpop_mod;

lalrpop_mod!(
    #[allow(clippy::all)]
    pub simplexpr_parser
);

pub use parser::parse_string;
