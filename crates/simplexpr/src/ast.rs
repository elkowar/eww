use crate::dynval::DynVal;
use eww_shared_util::{Span, DUMMY_SPAN};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use eww_shared_util::VarName;

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
    VarRef(Span, VarName),
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

    /// Construct a synthetic simplexpr from a literal string, without adding any relevant span information (uses [DUMMY_SPAN])
    pub fn synth_string(s: String) -> Self {
        Self::Literal(DUMMY_SPAN, DynVal(s, Some(DUMMY_SPAN)))
    }

    /// Construct a synthetic simplexpr from a literal dynval, without adding any relevant span information (uses [DUMMY_SPAN])
    pub fn synth_literal<T: Into<DynVal>>(s: T) -> Self {
        Self::Literal(DUMMY_SPAN, s.into())
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
