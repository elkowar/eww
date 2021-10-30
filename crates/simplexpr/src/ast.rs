use crate::dynval::DynVal;
use eww_shared_util::{Span, Spanned};
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
    Literal(DynVal),
    JsonArray(Span, Vec<SimplExpr>),
    JsonObject(Span, Vec<(SimplExpr, SimplExpr)>),
    Concat(Span, Vec<SimplExpr>),
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
            SimplExpr::Literal(x) => write!(f, "\"{}\"", x),
            SimplExpr::Concat(_, elems) => {
                let text = elems
                    .iter()
                    .map(|x| match x {
                        SimplExpr::Literal(lit) => lit.to_string(),
                        other => format!("${{{}}}", other),
                    })
                    .join("");
                write!(f, "\"{}\"", text)
            }
            SimplExpr::VarRef(_, x) => write!(f, "{}", x),
            SimplExpr::BinOp(_, l, op, r) => write!(f, "({} {} {})", l, op, r),
            SimplExpr::UnaryOp(_, op, x) => write!(f, "{}{}", op, x),
            SimplExpr::IfElse(_, a, b, c) => write!(f, "({} ? {} : {})", a, b, c),
            SimplExpr::JsonAccess(_, value, index) => write!(f, "{}[{}]", value, index),
            SimplExpr::FunctionCall(_, function_name, args) => {
                write!(f, "{}({})", function_name, args.iter().join(", "))
            }
            SimplExpr::JsonArray(_, values) => write!(f, "[{}]", values.iter().join(", ")),
            SimplExpr::JsonObject(_, entries) => {
                write!(f, "{{{}}}", entries.iter().map(|(k, v)| format!("{}: {}", k, v)).join(", "))
            }
        }
    }
}
impl SimplExpr {
    pub fn literal(span: Span, s: String) -> Self {
        Self::Literal(DynVal(s, span))
    }

    /// Construct a synthetic simplexpr from a literal string, without adding any relevant span information (uses [DUMMY_SPAN])
    pub fn synth_string(s: String) -> Self {
        Self::Literal(DynVal(s, Span::DUMMY))
    }

    /// Construct a synthetic simplexpr from a literal dynval, without adding any relevant span information (uses [DUMMY_SPAN])
    pub fn synth_literal<T: Into<DynVal>>(s: T) -> Self {
        Self::Literal(s.into())
    }

    pub fn collect_var_refs_into(&self, dest: &mut Vec<VarName>) {
        match self {
            SimplExpr::VarRef(_, x) => dest.push(x.clone()),
            SimplExpr::UnaryOp(_, _, x) => x.as_ref().collect_var_refs_into(dest),
            SimplExpr::BinOp(_, l, _, r) => {
                l.as_ref().collect_var_refs_into(dest);
                r.as_ref().collect_var_refs_into(dest);
            }
            SimplExpr::IfElse(_, a, b, c) => {
                a.as_ref().collect_var_refs_into(dest);
                b.as_ref().collect_var_refs_into(dest);
                c.as_ref().collect_var_refs_into(dest);
            }
            SimplExpr::JsonAccess(_, value, index) => {
                value.as_ref().collect_var_refs_into(dest);
                index.as_ref().collect_var_refs_into(dest);
            }
            SimplExpr::FunctionCall(_, _, args) => args.iter().for_each(|x: &SimplExpr| x.collect_var_refs_into(dest)),
            SimplExpr::JsonArray(_, values) => values.iter().for_each(|v| v.collect_var_refs_into(dest)),
            SimplExpr::JsonObject(_, entries) => entries.iter().for_each(|(k, v)| {
                k.collect_var_refs_into(dest);
                v.collect_var_refs_into(dest);
            }),
            SimplExpr::Concat(_, values) => values.iter().for_each(|x| x.collect_var_refs_into(dest)),
            SimplExpr::Literal(_) => {}
        };
    }

    pub fn collect_var_refs(&self) -> Vec<VarName> {
        let mut dest = Vec::new();
        self.collect_var_refs_into(&mut dest);
        dest
    }
}

impl Spanned for SimplExpr {
    fn span(&self) -> Span {
        match self {
            SimplExpr::Literal(x) => x.span(),
            SimplExpr::JsonArray(span, _) => *span,
            SimplExpr::JsonObject(span, _) => *span,
            SimplExpr::Concat(span, _) => *span,
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
