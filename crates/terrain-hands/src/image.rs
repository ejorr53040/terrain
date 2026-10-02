//! Borrowed camera images, and the sampling both models' inputs are built with.

use glam::Vec2;

/// A borrowed RGBA image, row-major, 8 bits per channel.
#[derive(Clone, Copy)]
pub struct RgbaImage<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: &'a [u8],
}

impl RgbaImage<'_> {
    /// Width and height, in pixels.
    pub fn size(&self) -> Vec2 {
        Vec2::new(self.width as f32, self.height as f32)
    }

    /// Whether `p` (pixels; pixel centers at +0.5) falls on the image.
    pub(crate) fn covers(&self, p: Vec2) -> bool {
        p.x >= 0.0 && p.y >= 0.0 && p.x <= self.width as f32 && p.y <= self.height as f32
    }

    /// RGB at `p` (pixels; pixel centers at +0.5), 0..1, bilinear, with
    /// edge pixels repeated beyond the border.
    pub(crate) fn sample(&self, p: Vec2) -> [f32; 3] {
        let (w, h) = (self.width as usize, self.height as usize);
        let p = (p - 0.5).clamp(Vec2::ZERO, Vec2::new(w as f32 - 1.0, h as f32 - 1.0));
        let (x0, y0) = (p.x as usize, p.y as usize);
        let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
        let (fx, fy) = (p.x - x0 as f32, p.y - y0 as f32);
        let at = |x: usize, y: usize, c: usize| self.pixels[(y * w + x) * 4 + c] as f32 / 255.0;
        std::array::from_fn(|c| {
            let top = at(x0, y0, c) * (1.0 - fx) + at(x1, y0, c) * fx;
            let bottom = at(x0, y1, c) * (1.0 - fx) + at(x1, y1, c) * fx;
            top * (1.0 - fy) + bottom * fy
        })
    }
}
