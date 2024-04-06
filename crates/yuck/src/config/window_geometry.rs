use std::collections::HashMap;

use crate::{
    enum_parse,
    error::DiagResult,
    format_diagnostic::ToDiagnostic,
    parser::{ast::Ast, ast_iterator::AstIterator, from_ast::FromAstElementContent},
    value::{coords, Coords, NumWithUnit},
};

use super::window_definition::EnumParseError;
use eww_shared_util::{Span, VarName};
use serde::{Deserialize, Serialize};
use simplexpr::{
    dynval::{DynVal, FromDynVal},
    eval::EvalError,
    SimplExpr,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, smart_default::SmartDefault, Serialize, Deserialize, strum::Display)]
pub enum AnchorAlignment {
    #[strum(serialize = "start")]
    #[default]
    START,
    #[strum(serialize = "center")]
    CENTER,
    #[strum(serialize = "end")]
    END,
}

impl AnchorAlignment {
    pub fn from_x_alignment(s: &str) -> Result<AnchorAlignment, EnumParseError> {
        enum_parse! { "x-alignment", s,
            "l" | "left" => AnchorAlignment::START,
            "c" | "center" => AnchorAlignment::CENTER,
            "r" | "right" => AnchorAlignment::END,
        }
    }

    pub fn from_y_alignment(s: &str) -> Result<AnchorAlignment, EnumParseError> {
        enum_parse! { "y-alignment", s,
            "t" | "top" => AnchorAlignment::START,
            "c" | "center" => AnchorAlignment::CENTER,
            "b" | "bottom" => AnchorAlignment::END,
        }
    }

    pub fn alignment_to_coordinate(&self, size_inner: i32, size_container: i32) -> i32 {
        match self {
            AnchorAlignment::START => 0,
            AnchorAlignment::CENTER => (size_container / 2) - (size_inner / 2),
            AnchorAlignment::END => size_container - size_inner,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct AnchorPoint {
    pub x: AnchorAlignment,
    pub y: AnchorAlignment,
}

impl std::fmt::Display for AnchorPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use AnchorAlignment::*;
        match (self.x, self.y) {
            (CENTER, CENTER) => write!(f, "center"),
            (x, y) => write!(
                f,
                "{} {}",
                match x {
                    START => "left",
                    CENTER => "center",
                    END => "right",
                },
                match y {
                    START => "top",
                    CENTER => "center",
                    END => "bottom",
                }
            ),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AnchorPointParseError {
    #[error("Could not parse anchor: Must either be \"center\" or be formatted like \"top left\"")]
    WrongFormat(String),
    #[error(transparent)]
    EnumParseError(#[from] EnumParseError),
}

impl std::str::FromStr for AnchorPoint {
    type Err = AnchorPointParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "center" {
            Ok(AnchorPoint { x: AnchorAlignment::CENTER, y: AnchorAlignment::CENTER })
        } else {
            let (first, second) = s.split_once(' ').ok_or_else(|| AnchorPointParseError::WrongFormat(s.to_string()))?;
            let x_y_result: Result<_, EnumParseError> = (move || {
                Ok(AnchorPoint { x: AnchorAlignment::from_x_alignment(first)?, y: AnchorAlignment::from_y_alignment(second)? })
            })();
            x_y_result.or_else(|_| {
                Ok(AnchorPoint { x: AnchorAlignment::from_x_alignment(second)?, y: AnchorAlignment::from_y_alignment(first)? })
            })
        }
    }
}

/// Unevaluated variant of [`Coords`]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CoordsDef {
    pub x: Option<SimplExpr>,
    pub y: Option<SimplExpr>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    AnchorPointParseError(#[from] AnchorPointParseError),
    #[error(transparent)]
    CoordsError(#[from] coords::Error),
    #[error(transparent)]
    EvalError(#[from] EvalError),
}

impl CoordsDef {
    pub fn eval(&self, local_variables: &HashMap<VarName, DynVal>) -> Result<Coords, Error> {
        Ok(Coords {
            x: convert_to_num_with_unit(&self.x, local_variables)?,
            y: convert_to_num_with_unit(&self.y, local_variables)?,
        })
    }
}

fn convert_to_num_with_unit(
    opt_expr: &Option<SimplExpr>,
    local_variables: &HashMap<VarName, DynVal>,
) -> Result<NumWithUnit, Error> {
    Ok(match opt_expr {
        Some(expr) => NumWithUnit::from_dynval(&expr.eval(local_variables)?)?,
        None => NumWithUnit::default(),
    })
}

/// Unevaluated variant of [`WindowGeometry`]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WindowGeometryDef {
    pub anchor_point: Option<SimplExpr>,
    pub offset: CoordsDef,
    pub size: CoordsDef,
}

impl FromAstElementContent for WindowGeometryDef {
    const ELEMENT_NAME: &'static str = "geometry";

    fn from_tail<I: Iterator<Item = Ast>>(_span: Span, mut iter: AstIterator<I>) -> DiagResult<Self> {
        let mut attrs = iter.expect_key_values()?;
        iter.expect_done()
            .map_err(|e| e.to_diagnostic().with_notes(vec!["Check if you are missing a colon in front of a key".to_string()]))?;

        Ok(WindowGeometryDef {
            anchor_point: attrs.ast_optional("anchor")?,
            size: CoordsDef { x: attrs.ast_optional("width")?, y: attrs.ast_optional("height")? },
            offset: CoordsDef { x: attrs.ast_optional("x")?, y: attrs.ast_optional("y")? },
        })
    }
}

impl WindowGeometryDef {
    pub fn eval(&self, local_variables: &HashMap<VarName, DynVal>) -> Result<WindowGeometry, Error> {
        Ok(WindowGeometry {
            anchor_point: match &self.anchor_point {
                Some(expr) => AnchorPoint::from_dynval(&expr.eval(local_variables)?)?,
                None => AnchorPoint::default(),
            },
            size: self.size.eval(local_variables)?,
            offset: self.offset.eval(local_variables)?,
        })
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize)]
pub struct WindowGeometry {
    pub anchor_point: AnchorPoint,
    pub offset: Coords,
    pub size: Coords,
}

impl WindowGeometry {
    pub fn override_if_given(&self, anchor_point: Option<AnchorPoint>, offset: Option<Coords>, size: Option<Coords>) -> Self {
        WindowGeometry {
            anchor_point: anchor_point.unwrap_or(self.anchor_point),
            offset: offset.unwrap_or(self.offset),
            size: size.unwrap_or(self.size),
        }
    }
}

impl std::fmt::Display for WindowGeometry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{} ({})", self.offset, self.size, self.anchor_point)
    }
}
