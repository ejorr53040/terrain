//! Shapes fixture for terrain-blocks.

mod geometry;

pub use geometry::Point;

/// A square.
#[derive(Debug, Clone, Copy)]
pub struct Square {
    pub side: f32,
}

impl Square {
    /// Area of the square.
    pub fn area(&self) -> f32 {
        self.side * self.side
    }

    pub fn perimeter(&self) -> f32 {
        4.0 * self.side
    }
}

// Shapes are measured in metres.
pub const UNIT: &str = "m";

pub mod util {
    pub fn double(x: f32) -> f32 {
        x * 2.0
    }
}

pub fn largest(a: Square, b: Square) -> Square {
    if a.area() >= b.area() { a } else { b }
}
