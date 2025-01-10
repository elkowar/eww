use std::{collections::HashMap, str::FromStr};

use anyhow::Result;
use simplexpr::{
    dynval::{DynVal, FromDynVal},
    eval::EvalError,
    SimplExpr,
};

use super::{attributes::Attributes, window_definition::EnumParseError};
use crate::{
    enum_parse,
    error::DiagResult,
    parser::{ast::Ast, ast_iterator::AstIterator, from_ast::FromAstElementContent},
    value::{coords, NumWithUnit},
};
use eww_shared_util::{Span, VarName};
use simplexpr::dynval::ConversionError;

use crate::error::{DiagError, DiagResultExt};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    EnumParseError(#[from] EnumParseError),
    #[error(transparent)]
    CoordsError(#[from] coords::Error),
    #[error(transparent)]
    EvalError(#[from] EvalError),
    #[error(transparent)]
    ConversionError(#[from] ConversionError),
}

/// Backend-specific options of a window
/// Unevaluated form of [`BackendWindowOptions`]
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct BackendWindowOptionsDef {
    pub wayland: WlBackendWindowOptionsDef,
    pub x11: X11BackendWindowOptionsDef,
}

impl BackendWindowOptionsDef {
    pub fn eval(&self, local_variables: &HashMap<VarName, DynVal>) -> Result<BackendWindowOptions, Error> {
        Ok(BackendWindowOptions { wayland: self.wayland.eval(local_variables)?, x11: self.x11.eval(local_variables)? })
    }

    pub fn from_attrs(attrs: &mut Attributes) -> DiagResult<Self> {
        let struts = attrs.ast_optional("reserve")?;
        let window_type = attrs.ast_optional("windowtype")?;
        let focusable = attrs.ast_optional("focusable")?;
        let x11 = X11BackendWindowOptionsDef {
            sticky: attrs.ast_optional("sticky")?,
            struts,
            window_type,
            wm_ignore: attrs.ast_optional("wm-ignore")?,
        };
        let wayland = WlBackendWindowOptionsDef {
            exclusive: attrs.ast_optional("exclusive")?,
            focusable,
            namespace: attrs.ast_optional("namespace")?,
        };

        Ok(Self { wayland, x11 })
    }
}

/// Backend-specific options of a window that are backend
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct BackendWindowOptions {
    pub x11: X11BackendWindowOptions,
    pub wayland: WlBackendWindowOptions,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct X11BackendWindowOptions {
    pub wm_ignore: bool,
    pub sticky: bool,
    pub window_type: X11WindowType,
    pub struts: X11StrutDefinition,
}

/// Unevaluated form of [`X11BackendWindowOptions`]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct X11BackendWindowOptionsDef {
    pub sticky: Option<SimplExpr>,
    pub struts: Option<X11StrutDefinitionExpr>,
    pub window_type: Option<SimplExpr>,
    pub wm_ignore: Option<SimplExpr>,
}

impl X11BackendWindowOptionsDef {
    fn eval(&self, local_variables: &HashMap<VarName, DynVal>) -> Result<X11BackendWindowOptions, Error> {
        Ok(X11BackendWindowOptions {
            sticky: eval_opt_expr_as_bool(&self.sticky, true, local_variables)?,
            struts: match &self.struts {
                Some(expr) => expr.eval(local_variables)?,
                None => X11StrutDefinition::default(),
            },
            window_type: match &self.window_type {
                Some(expr) => X11WindowType::from_dynval(&expr.eval(local_variables)?)?,
                None => X11WindowType::default(),
            },
            wm_ignore: eval_opt_expr_as_bool(
                &self.wm_ignore,
                self.window_type.is_none() && self.struts.is_none(),
                local_variables,
            )?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WlBackendWindowOptions {
    pub exclusive: bool,
    pub focusable: WlWindowFocusable,
    pub namespace: Option<String>,
}

/// Unevaluated form of [`WlBackendWindowOptions`]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WlBackendWindowOptionsDef {
    pub exclusive: Option<SimplExpr>,
    pub focusable: Option<SimplExpr>,
    pub namespace: Option<SimplExpr>,
}

impl WlBackendWindowOptionsDef {
    fn eval(&self, local_variables: &HashMap<VarName, DynVal>) -> Result<WlBackendWindowOptions, Error> {
        Ok(WlBackendWindowOptions {
            exclusive: eval_opt_expr_as_bool(&self.exclusive, false, local_variables)?,
            focusable: match &self.focusable {
                Some(expr) => WlWindowFocusable::from_dynval(&expr.eval(local_variables)?)?,
                None => WlWindowFocusable::default(),
            },
            namespace: match &self.namespace {
                Some(expr) => Some(expr.eval(local_variables)?.as_string()?),
                None => None,
            },
        })
    }
}

fn eval_opt_expr_as_bool(
    opt_expr: &Option<SimplExpr>,
    default: bool,
    local_variables: &HashMap<VarName, DynVal>,
) -> Result<bool, EvalError> {
    Ok(match opt_expr {
        Some(expr) => expr.eval(local_variables)?.as_bool()?,
        None => default,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, smart_default::SmartDefault, serde::Serialize)]
pub enum WlWindowFocusable {
    #[default]
    None,
    Exclusive,
    OnDemand,
}
impl FromStr for WlWindowFocusable {
    type Err = EnumParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        enum_parse! { "focusable", s,
            "none" => Self::None,
            "exclusive" => Self::Exclusive,
            "ondemand" => Self::OnDemand,
            // legacy support
            "true" => Self::Exclusive,
            "false" => Self::None,
        }
    }
}

/// Window type of an x11 window
#[derive(Debug, Clone, PartialEq, Eq, smart_default::SmartDefault, serde::Serialize)]
pub enum X11WindowType {
    #[default]
    Dock,
    Dialog,
    Toolbar,
    Normal,
    Utility,
    Desktop,
    Notification,
}
impl FromStr for X11WindowType {
    type Err = EnumParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        enum_parse! { "window type", s,
            "dock" => Self::Dock,
            "toolbar" => Self::Toolbar,
            "dialog" => Self::Dialog,
            "normal" => Self::Normal,
            "utility" => Self::Utility,
            "desktop" => Self::Desktop,
            "notification" => Self::Notification,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, smart_default::SmartDefault, serde::Serialize)]
pub enum Side {
    #[default]
    Top,
    Left,
    Right,
    Bottom,
}

impl FromStr for Side {
    type Err = EnumParseError;

    fn from_str(s: &str) -> Result<Side, Self::Err> {
        enum_parse! { "side", s,
            "l" | "left" => Side::Left,
            "r" | "right" => Side::Right,
            "t" | "top" => Side::Top,
            "b" | "bottom" => Side::Bottom,
        }
    }
}

/// Unevaluated form of [`X11StrutDefinition`]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct X11StrutDefinitionExpr {
    pub side: Option<SimplExpr>,
    pub distance: SimplExpr,
}

impl X11StrutDefinitionExpr {
    fn eval(&self, local_variables: &HashMap<VarName, DynVal>) -> Result<X11StrutDefinition, Error> {
        Ok(X11StrutDefinition {
            side: match &self.side {
                Some(expr) => Side::from_dynval(&expr.eval(local_variables)?)?,
                None => Side::default(),
            },
            distance: NumWithUnit::from_dynval(&self.distance.eval(local_variables)?)?,
        })
    }
}

impl FromAstElementContent for X11StrutDefinitionExpr {
    const ELEMENT_NAME: &'static str = "struts";

    fn from_tail<I: Iterator<Item = Ast>>(_span: Span, mut iter: AstIterator<I>) -> DiagResult<Self> {
        let mut attrs = iter.expect_key_values()?;
        iter.expect_done().map_err(DiagError::from).note("Check if you are missing a colon in front of a key")?;
        Ok(X11StrutDefinitionExpr { side: attrs.ast_optional("side")?, distance: attrs.ast_required("distance")? })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize)]
pub struct X11StrutDefinition {
    pub side: Side,
    pub distance: NumWithUnit,
}
