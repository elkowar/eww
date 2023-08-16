use std::{collections::HashMap, fmt::Display};

use crate::{
    config::monitor::MonitorIdentifier,
    error::{DiagError, DiagResult},
    parser::{
        ast::Ast,
        ast_iterator::AstIterator,
        from_ast::{FromAst, FromAstElementContent},
    },
};
use eww_shared_util::{Span, VarName};
use simplexpr::{
    dynval::{DynVal, FromDynVal},
    eval::EvalError,
    SimplExpr,
};

use super::{
    attributes::AttrSpec, backend_window_options::BackendWindowOptionsDef, widget_use::WidgetUse,
    window_geometry::WindowGeometryDef,
};

#[derive(Debug, thiserror::Error)]
pub enum WindowStackingConversionError {
    #[error(transparent)]
    EvalError(#[from] EvalError),
    #[error(transparent)]
    EnumParseError(#[from] EnumParseError),
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct WindowDefinition {
    pub name: String,
    pub expected_args: Vec<AttrSpec>,
    pub args_span: Span,
    pub geometry: Option<WindowGeometryDef>,
    pub stacking: Option<SimplExpr>,
    pub monitor: Option<SimplExpr>,
    pub widget: WidgetUse,
    pub resizable: Option<SimplExpr>,
    pub backend_options: BackendWindowOptionsDef,
}

impl WindowDefinition {
    /// Evaluate the `monitor` field of the window definition
    pub fn eval_monitor(&self, local_variables: &HashMap<VarName, DynVal>) -> Result<Option<MonitorIdentifier>, EvalError> {
        Ok(match &self.monitor {
            Some(monitor_expr) => Some(MonitorIdentifier::from_dynval(&monitor_expr.eval(local_variables)?)?),
            None => None,
        })
    }

    /// Evaluate the `resizable` field of the window definition
    pub fn eval_resizable(&self, local_variables: &HashMap<VarName, DynVal>) -> Result<bool, EvalError> {
        Ok(match &self.resizable {
            Some(expr) => expr.eval(local_variables)?.as_bool()?,
            None => true,
        })
    }

    /// Evaluate the `stacking` field of the window definition
    pub fn eval_stacking(
        &self,
        local_variables: &HashMap<VarName, DynVal>,
    ) -> Result<WindowStacking, WindowStackingConversionError> {
        match &self.stacking {
            Some(stacking_expr) => match stacking_expr.eval(local_variables) {
                Ok(val) => Ok(WindowStacking::from_dynval(&val)?),
                Err(err) => Err(WindowStackingConversionError::EvalError(err)),
            },
            None => Ok(WindowStacking::Foreground),
        }
    }
}

impl FromAstElementContent for WindowDefinition {
    const ELEMENT_NAME: &'static str = "defwindow";

    fn from_tail<I: Iterator<Item = Ast>>(_span: Span, mut iter: AstIterator<I>) -> DiagResult<Self> {
        let (_, name) = iter.expect_symbol()?;
        let (args_span, expected_args) = iter.expect_array().unwrap_or((Span::DUMMY, Vec::new()));
        let expected_args = expected_args.into_iter().map(AttrSpec::from_ast).collect::<DiagResult<_>>()?;
        let mut attrs = iter.expect_key_values()?;
        let monitor = attrs.ast_optional("monitor")?;
        let resizable = attrs.ast_optional("resizable")?;
        let stacking = attrs.ast_optional("stacking")?;
        let geometry = attrs.ast_optional("geometry")?;
        let backend_options = BackendWindowOptionsDef::from_attrs(&mut attrs)?;
        let widget = iter.expect_any().map_err(DiagError::from).and_then(WidgetUse::from_ast)?;
        iter.expect_done()?;
        Ok(Self { name, expected_args, args_span, monitor, resizable, widget, stacking, geometry, backend_options })
    }
}

#[derive(Debug, thiserror::Error)]
pub struct EnumParseError {
    pub input: String,
    pub expected: Vec<&'static str>,
}
impl Display for EnumParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse `{}`, must be one of {}", self.input, self.expected.join(", "))
    }
}

/// Parse a string with a concrete set of options into some data-structure,
/// and return an [EnumParseError]
/// ```rs
/// let input = "up";
/// enum_parse { "direction", input,
///   "up" => Direction::Up,
///   "down" => Direction::Down,
/// }
/// ```
#[macro_export]
macro_rules! enum_parse {
    ($name:literal, $input:expr, $($($s:literal)|* => $val:expr),* $(,)?) => {
        let input = $input.to_lowercase();
        match input.as_str() {
            $( $( $s )|* => Ok($val) ),*,
            _ => Err(EnumParseError {
                input,
                expected: vec![$($($s),*),*],
            })
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, derive_more::Display, smart_default::SmartDefault, serde::Serialize)]
pub enum WindowStacking {
    #[default]
    Foreground,
    Background,
    Bottom,
    Overlay,
}

impl std::str::FromStr for WindowStacking {
    type Err = EnumParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        enum_parse! { "WindowStacking", s,
            "foreground" | "fg" => WindowStacking::Foreground,
            "background" | "bg" => WindowStacking::Background,
            "bottom" | "bt" => WindowStacking::Bottom,
            "overlay" | "ov" => WindowStacking::Overlay,
        }
    }
}
