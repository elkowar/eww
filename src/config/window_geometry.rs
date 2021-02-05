use crate::{geometry, value::Coords};

use anyhow::*;
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
    pub fn get_window_rectangle_on(&self, rectangular: &impl geometry::Rectangular) -> geometry::Rect {
        let rect = rectangular.get_rect();

        let (offset_x, offset_y) = self.offset.relative_to(rect.width as i32, rect.height as i32);
        let (width, height) = self.size.relative_to(rect.width as i32, rect.height as i32);
        let x = rect.x + offset_x + self.anchor_point.x.alignment_to_coordinate(width, rect.width as i32);
        let y = rect.y + offset_y + self.anchor_point.y.alignment_to_coordinate(height, rect.height as i32);
        geometry::Rect { x, y, width, height }
    }
}

#[cfg(test)]
mod test {
    use super::{geometry::*, *};
    use crate::value::NumWithUnit;
    use pretty_assertions::assert_eq;

    #[test]
    pub fn test_get_window_rectangle_on_alignment() {
        fn make_aligned_window(anchor: AnchorAlignment) -> EwwWindowGeometry {
            EwwWindowGeometry {
                anchor_point: AnchorPoint { x: anchor, y: anchor },
                offset: Coords::from_pixels(1, 1),
                size: Coords::from_pixels(10, 10),
            }
        }

        let monitor = Rect::of(10, 10, 20, 20);

        assert_eq!(
            Rect::of(11, 11, 10, 10),
            make_aligned_window(AnchorAlignment::START).get_window_rectangle_on(&monitor)
        );
        assert_eq!(
            Rect::of(16, 16, 10, 10),
            make_aligned_window(AnchorAlignment::CENTER).get_window_rectangle_on(&monitor)
        );
        assert_eq!(
            Rect::of(21, 21, 10, 10),
            make_aligned_window(AnchorAlignment::END).get_window_rectangle_on(&monitor)
        );
    }
    #[test]
    pub fn test_get_window_rectangle_on_relative() {
        let window = EwwWindowGeometry {
            anchor_point: AnchorPoint {
                x: AnchorAlignment::START,
                y: AnchorAlignment::START,
            },
            offset: Coords {
                x: NumWithUnit::Percent(50),
                y: NumWithUnit::Pixels(0),
            },
            size: Coords {
                x: NumWithUnit::Percent(25),
                y: NumWithUnit::Percent(0),
            },
        };

        let monitor = Rect::of(0, 0, 100, 100);

        assert_eq!(Rect::of(50, 0, 25, 0), window.get_window_rectangle_on(monitor));
    }
}
