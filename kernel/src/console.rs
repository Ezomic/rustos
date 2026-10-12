use crate::framebuffer::{Color, Display};
use core::fmt;
use font8x8::legacy::BASIC_LEGACY;

const GLYPH: usize = 8;
const SCALE: usize = 2;
const CELL: usize = GLYPH * SCALE;
const FOREGROUND: Color = Color::new(0xdd, 0xdd, 0xdd);
const BACKGROUND: Color = Color::new(0x10, 0x12, 0x1a);
const CURSOR: Color = Color::new(0x6c, 0xc0, 0xff);

pub struct Console {
    display: Display,
    columns: usize,
    rows: usize,
    column: usize,
    row: usize,
}

impl Console {
    pub fn new(display: Display) -> Self {
        let columns = display.width() / CELL;
        let rows = display.height() / CELL;
        let mut console = Self {
            display,
            columns,
            rows,
            column: 0,
            row: 0,
        };
        console.clear();
        console
    }

    pub fn columns(&self) -> usize {
        self.columns
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn clear(&mut self) {
        self.display.fill_rect(
            0,
            0,
            self.display.width(),
            self.display.height(),
            BACKGROUND,
        );
        self.column = 0;
        self.row = 0;
        self.draw_cursor(CURSOR);
    }

    pub fn put_char(&mut self, ch: char) {
        self.draw_cursor(BACKGROUND);
        match ch {
            '\n' => self.new_line(),
            '\u{8}' => self.backspace(),
            _ => self.write_glyph(ch),
        }
        self.draw_cursor(CURSOR);
    }

    fn write_glyph(&mut self, ch: char) {
        if self.column == self.columns {
            self.new_line();
        }
        let index = if ch.is_ascii() {
            ch as usize
        } else {
            b'?' as usize
        };
        let glyph = BASIC_LEGACY[index];
        let origin_x = self.column * CELL;
        let origin_y = self.row * CELL;
        for (glyph_y, bits) in glyph.iter().enumerate() {
            for glyph_x in 0..GLYPH {
                let color = if bits >> glyph_x & 1 == 1 {
                    FOREGROUND
                } else {
                    BACKGROUND
                };
                self.display.fill_rect(
                    origin_x + glyph_x * SCALE,
                    origin_y + glyph_y * SCALE,
                    SCALE,
                    SCALE,
                    color,
                );
            }
        }
        self.column += 1;
    }

    fn backspace(&mut self) {
        if self.column == 0 {
            return;
        }
        self.column -= 1;
        self.display
            .fill_rect(self.column * CELL, self.row * CELL, CELL, CELL, BACKGROUND);
    }

    fn new_line(&mut self) {
        self.column = 0;
        if self.row + 1 < self.rows {
            self.row += 1;
        } else {
            self.display.scroll_up(CELL, self.rows * CELL, BACKGROUND);
        }
    }

    fn draw_cursor(&mut self, color: Color) {
        if self.column == self.columns {
            return;
        }
        self.display.fill_rect(
            self.column * CELL,
            self.row * CELL + CELL - SCALE * 2,
            CELL,
            SCALE * 2,
            color,
        );
    }
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        s.chars().for_each(|ch| self.put_char(ch));
        Ok(())
    }
}
