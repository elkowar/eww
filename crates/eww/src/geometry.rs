use derive_more::*;
pub trait Rectangular {
    fn get_rect(&self) -> Rect;
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Display)]
#[display(fmt = ".x*.y:.width*.height")]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn of(x: i32, y: i32, width: i32, height: i32) -> Self {
        Rect { x, y, width, height }
    }
}

impl Rectangular for Rect {
    fn get_rect(&self) -> Rect {
        *self
    }
}

impl Rectangular for gdk::Rectangle {
    fn get_rect(&self) -> Rect {
        Rect { x: self.x, y: self.y, width: self.width, height: self.height }
    }
}
