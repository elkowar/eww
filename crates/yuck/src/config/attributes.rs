use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
};

use simplexpr::{
    dynval::{DynVal, FromDynVal},
    eval::EvalError,
    SimplExpr,
};

use crate::{
    error::AstError,
    parser::{ast::Ast, from_ast::FromAst},
};
use eww_shared_util::{AttrName, Span, Spanned, VarName};

#[derive(Debug, thiserror::Error)]
pub enum AttrError {
    #[error("Missing required attribute {1}")]
    MissingRequiredAttr(Span, AttrName),

    #[error("{1}")]
    EvaluationError(Span, EvalError),

    #[error("{1}")]
    Other(Span, Box<dyn std::error::Error + Sync + Send + 'static>),
}

impl Spanned for AttrError {
    fn span(&self) -> Span {
        match self {
            AttrError::MissingRequiredAttr(span, _) => *span,
            AttrError::EvaluationError(span, _) => *span,
            AttrError::Other(span, _) => *span,
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
    pub value: Ast,
}

impl AttrEntry {
    pub fn new(key_span: Span, value: Ast) -> AttrEntry {
        AttrEntry { key_span, value }
    }
}

// TODO maybe make this generic over the contained content
#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize)]
pub struct Attributes {
    pub span: Span,
    pub attrs: HashMap<AttrName, AttrEntry>,
}

impl Attributes {
    pub fn new(span: Span, attrs: HashMap<AttrName, AttrEntry>) -> Self {
        Attributes { span, attrs }
    }

    pub fn ast_required<T: FromAst>(&mut self, key: &str) -> Result<T, AstError> {
        let key = AttrName(key.to_string());
        match self.attrs.remove(&key) {
            Some(AttrEntry { key_span, value }) => T::from_ast(value),
            None => Err(AttrError::MissingRequiredAttr(self.span, key.clone()).into()),
        }
    }

    pub fn ast_optional<T: FromAst>(&mut self, key: &str) -> Result<Option<T>, AstError> {
        match self.attrs.remove(&AttrName(key.to_string())) {
            Some(AttrEntry { key_span, value }) => T::from_ast(value).map(Some),
            None => Ok(None),
        }
    }

    /// Retrieve a required attribute from the set which _must not_ reference any variables,
    /// and is thus known to be static.
    pub fn primitive_required<T, E>(&mut self, key: &str) -> Result<T, AstError>
    where
        E: std::error::Error + 'static + Sync + Send,
        T: FromDynVal<Err = E>,
    {
        let ast: SimplExpr = self.ast_required(key)?;
        Ok(ast
            .eval_no_vars()
            .map_err(|err| AttrError::EvaluationError(ast.span(), err))?
            .read_as()
            .map_err(|e| AttrError::Other(ast.span(), Box::new(e)))?)
    }

    /// Retrieve an optional attribute from the set which _must not_ reference any variables,
    /// and is thus known to be static.
    pub fn primitive_optional<T, E>(&mut self, key: &str) -> Result<Option<T>, AstError>
    where
        E: std::error::Error + 'static + Sync + Send,
        T: FromDynVal<Err = E>,
    {
        let ast: SimplExpr = match self.ast_optional(key)? {
            Some(ast) => ast,
            None => return Ok(None),
        };
        Ok(Some(
            ast.eval_no_vars()
                .map_err(|err| AttrError::EvaluationError(ast.span(), err))?
                .read_as()
                .map_err(|e| AttrError::Other(ast.span(), Box::new(e)))?,
        ))
    }

    /// Consumes the attributes to return a list of unused attributes which may be used to emit a warning.
    /// TODO actually use this and implement warnings,... lol
    pub fn get_unused(self, definition_span: Span) -> UnusedAttrs {
        UnusedAttrs { definition_span, attrs: self.attrs.into_iter().map(|(k, v)| (v.key_span.to(v.value.span()), k)).collect() }
    }
}
