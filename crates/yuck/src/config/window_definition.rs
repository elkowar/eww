use std::{collections::HashMap, fmt::Display, str::FromStr};

use simplexpr::{dynval::DynVal, SimplExpr};

use crate::{
    config::monitor::MonitorIdentifier,
    error::{DiagError, DiagResult},
    format_diagnostic::ToDiagnostic,
    parser::{
        ast::Ast,
        ast_iterator::AstIterator,
        from_ast::{FromAst, FromAstElementContent},
    },
    value::NumWithUnit,
};
use eww_shared_util::{AttrName, Span, VarName};

use super::{backend_window_options::BackendWindowOptions, widget_use::WidgetUse, window_geometry::WindowGeometry};

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct WindowDefinition {
    pub name: String,
    pub geometry: Option<WindowGeometry>,
    pub stacking: WindowStacking,
    pub monitor: Option<MonitorIdentifier>,
    pub widget: WidgetUse,
    pub resizable: bool,
    pub backend_options: BackendWindowOptions,
}

impl FromAstElementContent for WindowDefinition {
    const ELEMENT_NAME: &'static str = "defwindow";

    fn from_tail<I: Iterator<Item = Ast>>(span: Span, mut iter: AstIterator<I>) -> DiagResult<Self> {
        let (_, name) = iter.expect_symbol()?;
        let mut attrs = iter.expect_key_values()?;
        let monitor = attrs.primitive_optional("monitor")?;
        let resizable = attrs.primitive_optional("resizable")?.unwrap_or(true);
        let stacking = attrs.primitive_optional("stacking")?.unwrap_or(WindowStacking::Foreground);
        let geometry = attrs.ast_optional("geometry")?;
        let backend_options = BackendWindowOptions::from_attrs(&mut attrs)?;
        let widget = iter.expect_any().map_err(DiagError::from).and_then(WidgetUse::from_ast)?;
        iter.expect_done()?;
        Ok(Self { name, monitor, resizable, widget, stacking, geometry, backend_options })
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
