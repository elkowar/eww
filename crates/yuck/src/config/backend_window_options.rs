use std::str::FromStr;

use anyhow::{anyhow, Result};

use crate::{
    enum_parse,
    error::DiagResult,
    parser::{ast::Ast, ast_iterator::AstIterator, from_ast::FromAstElementContent},
    value::NumWithUnit,
};
use eww_shared_util::Span;

use super::{attributes::Attributes, window_definition::EnumParseError};

pub use backend::*;

#[cfg(feature = "x11")]
mod backend {
    use crate::{
        error::{DiagError, DiagResultExt},
        format_diagnostic::ToDiagnostic,
    };

    use super::*;

    #[derive(Debug, Clone, PartialEq, serde::Serialize)]
    pub struct BackendWindowOptions {
        pub wm_ignore: bool,
        pub sticky: bool,
        pub window_type: WindowType,
        pub struts: StrutDefinition,
    }

    impl BackendWindowOptions {
        pub fn from_attrs(attrs: &mut Attributes) -> DiagResult<Self> {
            let struts = attrs.ast_optional("reserve")?;
            let window_type = attrs.primitive_optional("windowtype")?;
            Ok(Self {
                wm_ignore: attrs.primitive_optional("wm-ignore")?.unwrap_or(window_type.is_none() && struts.is_none()),
                window_type: window_type.unwrap_or_default(),
                sticky: attrs.primitive_optional("sticky")?.unwrap_or(true),
                struts: struts.unwrap_or_default(),
            })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, smart_default::SmartDefault, serde::Serialize)]
    pub enum WindowType {
        #[default]
        Dock,
        Dialog,
        Toolbar,
        Normal,
        Utility,
        Desktop,
        Notification,
    }
    impl FromStr for WindowType {
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
    #[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize)]
    pub struct StrutDefinition {
        pub side: Side,
        pub dist: NumWithUnit,
    }

    impl FromAstElementContent for StrutDefinition {
        const ELEMENT_NAME: &'static str = "struts";

        fn from_tail<I: Iterator<Item = Ast>>(span: Span, mut iter: AstIterator<I>) -> DiagResult<Self> {
            let mut attrs = iter.expect_key_values()?;
            iter.expect_done().map_err(DiagError::from).note("Check if you are missing a colon in front of a key")?;
            Ok(StrutDefinition { side: attrs.primitive_required("side")?, dist: attrs.primitive_required("distance")? })
        }
    }
}

#[cfg(feature = "wayland")]
mod backend {
    use super::*;
    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
    pub struct BackendWindowOptions {
        pub exclusive: bool,
        pub focusable: bool,
    }
    impl BackendWindowOptions {
        pub fn from_attrs(attrs: &mut Attributes) -> DiagResult<Self> {
            Ok(Self {
                exclusive: attrs.primitive_optional("exclusive")?.unwrap_or(false),
                focusable: attrs.primitive_optional("focusable")?.unwrap_or(false),
            })
        }
    }
}

#[cfg(not(any(feature = "x11", feature = "wayland")))]
mod backend {
    use super::*;
    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
    pub struct BackendWindowOptions;
    impl BackendWindowOptions {
        pub fn from_attrs(attrs: &mut Attributes) -> DiagResult<Self> {
            Ok(Self)
        }
    }
}
