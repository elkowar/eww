use std::{collections::HashMap, fmt::Display, str::FromStr};

use simplexpr::{dynval::DynVal, SimplExpr};

use crate::{
    error::{AstError, AstResult},
    parser::{
        ast::{Ast, AstIterator, Span},
        from_ast::{FromAst, FromAstElementContent},
    },
    spanned,
    value::{AttrName, NumWithUnit, VarName},
};

use super::{widget_use::WidgetUse, window_geometry::WindowGeometry};

#[derive(Debug, Clone, serde::Serialize)]
pub struct EwwWindowDefinition {
    pub name: String,
    pub geometry: Option<WindowGeometry>,
    pub stacking: WindowStacking,
    pub monitor_number: Option<i32>,
    pub widget: WidgetUse,
    pub resizable: bool,
    // pub backend_options: BackendWindowOptions,
}

impl FromAstElementContent for EwwWindowDefinition {
    fn get_element_name() -> &'static str {
        "defwindow"
    }

    fn from_tail<I: Iterator<Item = Ast>>(span: Span, mut iter: AstIterator<I>) -> AstResult<Self> {
        let (_, name) = iter.expect_symbol()?;
        let mut attrs = iter.expect_key_values()?;
        let monitor_number = attrs.primitive_optional("monitor")?;
        let resizable = attrs.primitive_optional("resizable")?.unwrap_or(true);
        let stacking = attrs.primitive_optional("stacking")?.unwrap_or(WindowStacking::Foreground);
        let geometry = attrs.ast_optional("geometry")?;
        let widget = iter.expect_any()?;
        Ok(Self { name, monitor_number, resizable, widget, stacking, geometry })
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
                input: input,
                expected: vec![$($($s),*),*],
            })
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, smart_default::SmartDefault)]
pub enum EwwWindowType {
    #[default]
    Dock,
    Dialog,
    Toolbar,
    Normal,
    Utility,
}
impl FromStr for EwwWindowType {
    type Err = EnumParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        enum_parse! { "window type", s,
            "dock" => Self::Dock,
            "toolbar" => Self::Toolbar,
            "dialog" => Self::Dialog,
            "normal" => Self::Normal,
            "utility" => Self::Utility,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, smart_default::SmartDefault)]
pub enum Side {
    #[default]
    Top,
    Left,
    Right,
    Bottom,
}

impl std::str::FromStr for Side {
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

// Surface definition if the backend for X11 is enable
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub struct StrutDefinition {
    pub side: Side,
    pub dist: NumWithUnit,
}

impl FromAstElementContent for StrutDefinition {
    fn get_element_name() -> &'static str {
        "struts"
    }

    fn from_tail<I: Iterator<Item = Ast>>(span: Span, mut iter: AstIterator<I>) -> AstResult<Self> {
        let mut attrs = iter.expect_key_values()?;
        Ok(StrutDefinition { side: attrs.primitive_required("side")?, dist: attrs.primitive_required("distance")? })
    }
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
