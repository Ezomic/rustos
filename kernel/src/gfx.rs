use alloc::vec;
use alloc::vec::Vec;
use font8x8::legacy::BASIC_LEGACY;

pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub const fn right(&self) -> i32 {
        self.x + self.w
    }

    pub const fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub const fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    pub fn intersect(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        (right > x && bottom > y).then(|| Rect::new(x, y, right - x, bottom - y))
    }

    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect::new(
            x,
            y,
            self.right().max(other.right()) - x,
            self.bottom().max(other.bottom()) - y,
        )
    }
}

pub struct Canvas {
    width: i32,
    height: i32,
    pixels: Vec<u32>,
    clip: Rect,
}

impl Canvas {
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height) as usize],
            clip: Rect::new(0, 0, width, height),
        }
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    pub fn set_clip(&mut self, clip: Rect) {
        let bounds = Rect::new(0, 0, self.width, self.height);
        self.clip = clip.intersect(&bounds).unwrap_or(Rect::new(0, 0, 0, 0));
    }

    pub fn reset_clip(&mut self) {
        self.clip = Rect::new(0, 0, self.width, self.height);
    }

    pub fn fill_rect(&mut self, rect: Rect, color: u32) {
        let Some(area) = rect.intersect(&self.clip) else {
            return;
        };
        for y in area.y..area.bottom() {
            let start = (y * self.width + area.x) as usize;
            self.pixels[start..start + area.w as usize].fill(color);
        }
    }

    pub fn put_pixel(&mut self, x: i32, y: i32, color: u32) {
        if self.clip.contains(x, y) {
            self.pixels[(y * self.width + x) as usize] = color;
        }
    }

    pub fn blit(&mut self, source: &Canvas, x: i32, y: i32) {
        let target = Rect::new(x, y, source.width, source.height);
        let Some(area) = target.intersect(&self.clip) else {
            return;
        };
        for row in area.y..area.bottom() {
            let from = ((row - y) * source.width + (area.x - x)) as usize;
            let to = (row * self.width + area.x) as usize;
            self.pixels[to..to + area.w as usize]
                .copy_from_slice(&source.pixels[from..from + area.w as usize]);
        }
    }

    pub fn draw_glyph(&mut self, x: i32, y: i32, ch: char, color: u32, scale: i32) {
        let index = if ch.is_ascii() {
            ch as usize
        } else {
            b'?' as usize
        };
        for (glyph_y, bits) in BASIC_LEGACY[index].iter().enumerate() {
            for glyph_x in 0..8 {
                if bits >> glyph_x & 1 == 1 {
                    let rect = Rect::new(
                        x + glyph_x * scale,
                        y + glyph_y as i32 * scale,
                        scale,
                        scale,
                    );
                    self.fill_rect(rect, color);
                }
            }
        }
    }

    pub fn draw_text(&mut self, x: i32, y: i32, text: &str, color: u32, scale: i32) {
        for (index, ch) in text.chars().enumerate() {
            self.draw_glyph(x + index as i32 * 8 * scale, y, ch, color, scale);
        }
    }

    pub fn scroll_up(&mut self, lines: i32, background: u32) {
        let shift = (lines * self.width) as usize;
        self.pixels.copy_within(shift.., 0);
        self.fill_rect(
            Rect::new(0, self.height - lines, self.width, lines),
            background,
        );
    }
}
