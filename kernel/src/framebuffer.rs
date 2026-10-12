use crate::gfx::{Canvas, Rect};
use bootloader_api::info::{FrameBufferInfo, PixelFormat};

pub struct Display {
    buffer: &'static mut [u8],
    info: FrameBufferInfo,
}

impl Display {
    pub fn new(buffer: &'static mut [u8], info: FrameBufferInfo) -> Self {
        Self { buffer, info }
    }

    pub fn width(&self) -> i32 {
        self.info.width as i32
    }

    pub fn height(&self) -> i32 {
        self.info.height as i32
    }

    pub fn blit(&mut self, canvas: &Canvas, rect: Rect) {
        let bounds = Rect::new(0, 0, self.width(), self.height());
        let Some(area) = rect.intersect(&bounds) else {
            return;
        };
        let bpp = self.info.bytes_per_pixel;
        for y in area.y..area.bottom() {
            let from = (y * canvas.width() + area.x) as usize;
            let source = &canvas.pixels()[from..from + area.w as usize];
            let start = (y as usize * self.info.stride + area.x as usize) * bpp;
            let target = &mut self.buffer[start..start + area.w as usize * bpp];
            for (pixel, color) in target.chunks_exact_mut(bpp).zip(source) {
                let (r, g, b) = ((color >> 16) as u8, (color >> 8) as u8, *color as u8);
                match self.info.pixel_format {
                    PixelFormat::Rgb => pixel[..3].copy_from_slice(&[r, g, b]),
                    PixelFormat::Bgr => pixel[..3].copy_from_slice(&[b, g, r]),
                    PixelFormat::U8 => pixel[0] = ((r as u16 + g as u16 + b as u16) / 3) as u8,
                    _ => {}
                }
            }
        }
    }
}
