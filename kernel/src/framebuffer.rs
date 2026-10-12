use crate::gfx::{Canvas, Rect};
use alloc::format;
use alloc::string::String;
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

    pub fn summary(&self) -> String {
        format!(
            "{}x{} stride {} {} bytes/pixel {:?}",
            self.info.width,
            self.info.height,
            self.info.stride,
            self.info.bytes_per_pixel,
            self.info.pixel_format
        )
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
            let pixels = target.chunks_exact_mut(bpp).zip(source);
            match self.info.pixel_format {
                PixelFormat::Rgb => {
                    for (pixel, color) in pixels {
                        pixel[..3].copy_from_slice(&[
                            (color >> 16) as u8,
                            (color >> 8) as u8,
                            *color as u8,
                        ]);
                    }
                }
                PixelFormat::Bgr => {
                    for (pixel, color) in pixels {
                        pixel[..3].copy_from_slice(&[
                            *color as u8,
                            (color >> 8) as u8,
                            (color >> 16) as u8,
                        ]);
                    }
                }
                PixelFormat::U8 => {
                    for (pixel, color) in pixels {
                        let sum = (color >> 16 & 0xff) + (color >> 8 & 0xff) + (color & 0xff);
                        pixel[0] = (sum / 3) as u8;
                    }
                }
                _ => {}
            }
        }
    }
}
