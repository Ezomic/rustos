use bootloader_api::info::{FrameBufferInfo, PixelFormat};

#[derive(Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

pub struct Display {
    buffer: &'static mut [u8],
    info: FrameBufferInfo,
}

impl Display {
    pub fn new(buffer: &'static mut [u8], info: FrameBufferInfo) -> Self {
        Self { buffer, info }
    }

    pub fn width(&self) -> usize {
        self.info.width
    }

    pub fn height(&self) -> usize {
        self.info.height
    }

    pub fn put_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x >= self.info.width || y >= self.info.height {
            return;
        }
        let bpp = self.info.bytes_per_pixel;
        let offset = (y * self.info.stride + x) * bpp;
        let pixel = &mut self.buffer[offset..offset + bpp];
        match self.info.pixel_format {
            PixelFormat::Rgb => {
                pixel[0] = color.r;
                pixel[1] = color.g;
                pixel[2] = color.b;
            }
            PixelFormat::Bgr => {
                pixel[0] = color.b;
                pixel[1] = color.g;
                pixel[2] = color.r;
            }
            PixelFormat::U8 => {
                pixel[0] = ((color.r as u16 + color.g as u16 + color.b as u16) / 3) as u8;
            }
            _ => {}
        }
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: Color) {
        for row in y..y + h {
            for col in x..x + w {
                self.put_pixel(col, row, color);
            }
        }
    }

    pub fn scroll_up(&mut self, lines: usize, visible_height: usize, background: Color) {
        let row_bytes = self.info.stride * self.info.bytes_per_pixel;
        let total = visible_height * row_bytes;
        self.buffer.copy_within(lines * row_bytes..total, 0);
        self.fill_rect(
            0,
            visible_height - lines,
            self.info.width,
            lines,
            background,
        );
    }
}
