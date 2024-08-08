use derive_more::{Debug, *};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Display)]
#[display(".x*.y:.width*.height")]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
