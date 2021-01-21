use crate::value::Coords;
use anyhow::*;
use gtk4::gdk;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

use std::fmt;

use super::xml_ext::XmlElement;

#[derive(Debug, derive_more::Display, Clone, Copy, Eq, PartialEq, SmartDefault, Serialize, Deserialize)]
pub enum AnchorAlignment {
    #[display("start")]
    #[default]
    START,
    #[display("center")]
    CENTER,
    #[display("end")]
    END,
}

impl AnchorAlignment {
    pub fn from_x_alignment(s: &str) -> Result<AnchorAlignment> {
        match s {
            "l" | "left" => Ok(AnchorAlignment::START),
            "c" | "center" => Ok(AnchorAlignment::CENTER),
            "r" | "right" => Ok(AnchorAlignment::END),
            _ => bail!(
                r#"couldn't parse '{}' as x-alignment. Must be one of "left", "center", "right""#,
                s
            ),
        }
    }

    pub fn from_y_alignment(s: &str) -> Result<AnchorAlignment> {
        match s {
            "t" | "top" => Ok(AnchorAlignment::START),
            "c" | "center" => Ok(AnchorAlignment::CENTER),
            "b" | "bottom" => Ok(AnchorAlignment::END),
            _ => bail!(
                r#"couldn't parse '{}' as y-alignment. Must be one of "top", "center", "bottom""#,
                s
            ),
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
    x: AnchorAlignment,
    y: AnchorAlignment,
}

impl std::fmt::Display for AnchorPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl std::str::FromStr for AnchorPoint {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "center" {
            Ok(AnchorPoint {
                x: AnchorAlignment::CENTER,
                y: AnchorAlignment::CENTER,
            })
        } else {
            let (first, second) = s
                .split_once(' ')
                .context("Failed to parse anchor: Must either be \"center\" or be formatted like \"top left\"")?;
            let x_y_result: Result<_> = try {
                AnchorPoint {
                    x: AnchorAlignment::from_x_alignment(first)?,
                    y: AnchorAlignment::from_y_alignment(second)?,
                }
            };
            x_y_result.or_else(|_| {
                Ok(AnchorPoint {
                    x: AnchorAlignment::from_x_alignment(second)?,
                    y: AnchorAlignment::from_y_alignment(first)?,
                })
            })
        }
    }
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
pub struct EwwWindowGeometry {
    pub anchor_point: AnchorPoint,
    pub offset: Coords,
    pub size: Coords,
}

impl EwwWindowGeometry {
    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        Ok(EwwWindowGeometry {
            anchor_point: xml.parse_optional_attr("anchor")?.unwrap_or_default(),
            size: Coords {
                x: xml.parse_optional_attr("width")?.unwrap_or_default(),
                y: xml.parse_optional_attr("height")?.unwrap_or_default(),
            },
            offset: Coords {
                x: xml.parse_optional_attr("x")?.unwrap_or_default(),
                y: xml.parse_optional_attr("y")?.unwrap_or_default(),
            },
        })
    }

    pub fn override_if_given(&mut self, anchor_point: Option<AnchorPoint>, offset: Option<Coords>, size: Option<Coords>) -> Self {
        EwwWindowGeometry {
            anchor_point: anchor_point.unwrap_or(self.anchor_point),
            offset: offset.unwrap_or(self.offset),
            size: size.unwrap_or(self.size),
        }
    }
}

impl std::fmt::Display for EwwWindowGeometry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{} ({})", self.offset, self.size, self.anchor_point)
    }
}

impl EwwWindowGeometry {
    /// Calculate the window rectangle given the configured window geometry
    pub fn get_window_rectangle(&self, screen_rect: gdk::Rectangle) -> gdk::Rectangle {
        let (offset_x, offset_y) = self.offset.relative_to(screen_rect.width, screen_rect.height);
        let (width, height) = self.size.relative_to(screen_rect.width, screen_rect.height);
        let x = screen_rect.x + offset_x + self.anchor_point.x.alignment_to_coordinate(width, screen_rect.width);
        let y = screen_rect.y + offset_y + self.anchor_point.y.alignment_to_coordinate(height, screen_rect.height);
        gdk::Rectangle { x, y, width, height }
    }
}
