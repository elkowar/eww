use crate::dynval::DynVal;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

/// stores the left and right end of a span, and a given file identifier.
#[derive(Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub struct Span(pub usize, pub usize, pub usize);
pub static DUMMY_SPAN: Span = Span(usize::MAX, usize::MAX, usize::MAX);

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

#[rustfmt::skip]
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug, strum::EnumString, strum::Display)]
pub enum BinOp {
    #[strum(serialize = "+") ] Plus,
    #[strum(serialize = "-") ] Minus,
    #[strum(serialize = "*") ] Times,
    #[strum(serialize = "/") ] Div,
    #[strum(serialize = "%") ] Mod,
    #[strum(serialize = "==")] Equals,
    #[strum(serialize = "!=")] NotEquals,
    #[strum(serialize = "&&")] And,
    #[strum(serialize = "||")] Or,
    #[strum(serialize = ">") ] GT,
    #[strum(serialize = "<") ] LT,
    #[strum(serialize = "?:")] Elvis,
    #[strum(serialize = "=~")] RegexMatch,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug, strum::EnumString, strum::Display)]
pub enum UnaryOp {
    #[strum(serialize = "!")]
    Not,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimplExpr {
    Literal(Span, DynVal),
    VarRef(Span, String),
    BinOp(Span, Box<SimplExpr>, BinOp, Box<SimplExpr>),
    UnaryOp(Span, UnaryOp, Box<SimplExpr>),
    IfElse(Span, Box<SimplExpr>, Box<SimplExpr>, Box<SimplExpr>),
    JsonAccess(Span, Box<SimplExpr>, Box<SimplExpr>),
    FunctionCall(Span, String, Vec<SimplExpr>),
}

impl std::fmt::Display for SimplExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimplExpr::VarRef(_, x) => write!(f, "{}", x),
            SimplExpr::Literal(_, x) => write!(f, "\"{}\"", x),
            SimplExpr::BinOp(_, l, op, r) => write!(f, "({} {} {})", l, op, r),
            SimplExpr::UnaryOp(_, op, x) => write!(f, "{}{}", op, x),
            SimplExpr::IfElse(_, a, b, c) => write!(f, "(if {} then {} else {})", a, b, c),
            SimplExpr::JsonAccess(_, value, index) => write!(f, "{}[{}]", value, index),
            SimplExpr::FunctionCall(_, function_name, args) => {
                write!(f, "{}({})", function_name, args.iter().join(", "))
            }
        }
    }
}
impl SimplExpr {
    pub fn literal(span: Span, s: String) -> Self {
        Self::Literal(span, DynVal(s, Some(span)))
    }

    /// Construct a synthetic simplexpr from a literal value, without adding any relevant span information (uses [DUMMY_SPAN])
    pub fn synth_literal(s: String) -> Self {
        Self::Literal(DUMMY_SPAN, DynVal(s, Some(DUMMY_SPAN)))
    }

    pub fn span(&self) -> Span {
        match self {
            SimplExpr::Literal(span, _) => *span,
            SimplExpr::VarRef(span, _) => *span,
            SimplExpr::BinOp(span, ..) => *span,
            SimplExpr::UnaryOp(span, ..) => *span,
            SimplExpr::IfElse(span, ..) => *span,
            SimplExpr::JsonAccess(span, ..) => *span,
            SimplExpr::FunctionCall(span, ..) => *span,
        }
    }
}

impl std::fmt::Debug for SimplExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}
