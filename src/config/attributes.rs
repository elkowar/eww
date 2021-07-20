use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
};

use simplexpr::{dynval::DynVal, eval::EvalError, SimplExpr};

use crate::{
    parser::{
        ast::{Ast, Span},
        from_ast::FromAst,
    },
    value::AttrName,
};

#[derive(Debug, thiserror::Error)]
pub enum AttrError {
    #[error("Missing required attribute {0}")]
    MissingRequiredAttr(Span, AttrName),

    #[error("Failed to parse attribute value {0} in this context")]
    AttrTypeError(Span, AttrName),

    #[error("{1}")]
    EvaluationError(Span, EvalError),
}

impl AttrError {
    pub fn span(&self) -> Span {
        match self {
            AttrError::MissingRequiredAttr(span, _) => *span,
            AttrError::AttrTypeError(span, _) => *span,
            AttrError::EvaluationError(span, _) => *span,
        }
    }
}

#[derive(Debug)]
pub struct UnusedAttrs {
    definition_span: Span,
    attrs: Vec<(Span, AttrName)>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AttrEntry {
    pub key_span: Span,
    pub value: SimplExpr,
}

impl AttrEntry {
    pub fn new(key_span: Span, value: SimplExpr) -> AttrEntry {
        AttrEntry { key_span, value }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize)]
pub struct Attributes {
    pub span: Span,
    pub attrs: HashMap<AttrName, AttrEntry>,
}

impl Attributes {
    pub fn new(span: Span, attrs: HashMap<AttrName, AttrEntry>) -> Self {
        Attributes { span, attrs }
    }

    pub fn eval_required<T: TryFrom<DynVal>>(&mut self, key: &str) -> Result<T, AttrError> {
        let key = AttrName(key.to_string());
        match self.attrs.remove(&key) {
            Some(AttrEntry { key_span, value }) => {
                let value_span = value.span();
                let dynval = value.eval_no_vars().map_err(|err| AttrError::EvaluationError(value_span.into(), err))?;
                T::try_from(dynval).map_err(|_| AttrError::AttrTypeError(value_span.into(), key.clone()))
            }
            None => Err(AttrError::MissingRequiredAttr(self.span, key.clone())),
        }
    }

    pub fn eval_optional<T: TryFrom<DynVal>>(&mut self, key: &str) -> Result<Option<T>, AttrError> {
        let key = AttrName(key.to_string());
        match self.attrs.remove(&key) {
            Some(AttrEntry { key_span, value }) => {
                let value_span = value.span();
                let dynval = value.eval_no_vars().map_err(|err| AttrError::EvaluationError(value_span.into(), err))?;
                T::try_from(dynval).map(Some).map_err(|_| AttrError::AttrTypeError(value_span.into(), key.clone()))
            }
            None => Ok(None),
        }
    }

    // pub fn parse_required<T: TryFrom<SimplExpr>>(&mut self, key: &str) -> Result<T, AttrError> {
    // let key = AttrName(key.to_string());
    // match self.attrs.remove(&key) {
    // Some(value) => match value.value.try_into() {
    // Ok(value) => Ok(value),
    // Err(_) => Err(AttrError::AttrTypeError(value.value.span().into(), key.clone())),
    // },
    // None => Err(AttrError::MissingRequiredAttr(self.span, key.clone())),
    // }
    // }
    //
    // pub fn parse_optional<T: TryFrom<SimplExpr>>(&mut self, key: &str) -> Result<Option<T>, AttrError> {
    // let key = AttrName(key.to_string());
    // match self.attrs.remove(&key) {
    // Some(value) => match value.value.try_into() {
    // Ok(value) => Ok(Some(value)),
    // Err(_) => Err(AttrError::AttrTypeError(value.value.span().into(), key.clone())),
    // },
    // None => Ok(None),
    // }
    // }

    /// Consumes the attributes to return a list of unused attributes which may be used to emit a warning.
    /// TODO actually use this and implement warnings,... lol
    pub fn get_unused(self, definition_span: Span) -> UnusedAttrs {
        UnusedAttrs {
            definition_span,
            attrs: self.attrs.into_iter().map(|(k, v)| (v.key_span.to(v.value.span().into()), k)).collect(),
        }
    }
}
