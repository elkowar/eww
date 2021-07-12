use itertools::Itertools;
use serde::{Deserialize, Serialize};

#[derive(Eq, PartialEq, Clone, Copy)]
pub struct Span(pub usize, pub usize, pub usize);

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.0, self.1)
    }
}

impl std::fmt::Debug for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.0, self.1)
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum BinOp {
    Plus,
    Minus,
    Times,
    Div,
    Mod,
    Equals,
    NotEquals,
    And,
    Or,
    GT,
    LT,
    Elvis,
    RegexMatch,
}

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinOp::Plus => write!(f, "+"),
            BinOp::Minus => write!(f, "-"),
            BinOp::Times => write!(f, "*"),
            BinOp::Div => write!(f, "/"),
            BinOp::Mod => write!(f, "%"),
            BinOp::Equals => write!(f, "=="),
            BinOp::NotEquals => write!(f, "!="),
            BinOp::And => write!(f, "&&"),
            BinOp::Or => write!(f, "||"),
            BinOp::GT => write!(f, ">"),
            BinOp::LT => write!(f, "<"),
            BinOp::Elvis => write!(f, "?:"),
            BinOp::RegexMatch => write!(f, "=~"),
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum UnaryOp {
    Not,
}

impl std::fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnaryOp::Not => write!(f, "!"),
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum SimplExpr {
    Literal(String),
    VarRef(String),
    BinOp(Box<SimplExpr>, BinOp, Box<SimplExpr>),
    UnaryOp(UnaryOp, Box<SimplExpr>),
    IfElse(Box<SimplExpr>, Box<SimplExpr>, Box<SimplExpr>),
    JsonAccess(Box<SimplExpr>, Box<SimplExpr>),
    FunctionCall(String, Vec<SimplExpr>),
}

impl std::fmt::Display for SimplExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimplExpr::VarRef(x) => write!(f, "{}", x),
            SimplExpr::Literal(x) => write!(f, "\"{}\"", x),
            SimplExpr::BinOp(l, op, r) => write!(f, "({} {} {})", l, op, r),
            SimplExpr::UnaryOp(op, x) => write!(f, "{}{}", op, x),
            SimplExpr::IfElse(a, b, c) => write!(f, "(if {} then {} else {})", a, b, c),
            SimplExpr::JsonAccess(value, index) => write!(f, "{}[{}]", value, index),
            SimplExpr::FunctionCall(function_name, args) => {
                write!(f, "{}({})", function_name, args.iter().join(", "))
            }
        }
    }
}
