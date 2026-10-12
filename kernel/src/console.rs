use crate::gfx::{rgb, Canvas, Rect};
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;

const SCALE: i32 = 2;
const CELL: i32 = 8 * SCALE;
const FOREGROUND: u32 = rgb(0xdd, 0xdd, 0xdd);
const BACKGROUND: u32 = rgb(0x10, 0x12, 0x1a);
const CURSOR: u32 = rgb(0x6c, 0xc0, 0xff);

pub struct Console {
    canvas: Canvas,
    columns: i32,
    rows: i32,
    column: i32,
    row: i32,
}

impl Console {
    pub fn new(columns: i32, rows: i32) -> Self {
        let mut console = Self {
            canvas: Canvas::new(columns * CELL, rows * CELL),
            columns,
            rows,
            column: 0,
            row: 0,
        };
        console.clear();
        console
    }

    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    pub fn clear(&mut self) {
        self.canvas.fill_rect(
            Rect::new(0, 0, self.canvas.width(), self.canvas.height()),
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

    fn cell(&self) -> Rect {
        Rect::new(self.column * CELL, self.row * CELL, CELL, CELL)
    }

    fn write_glyph(&mut self, ch: char) {
        if self.column == self.columns {
            self.new_line();
        }
        let cell = self.cell();
        self.canvas.fill_rect(cell, BACKGROUND);
        self.canvas
            .draw_glyph(cell.x, cell.y, ch, FOREGROUND, SCALE);
        self.column += 1;
    }

    fn backspace(&mut self) {
        if self.column == 0 {
            return;
        }
        self.column -= 1;
        let cell = self.cell();
        self.canvas.fill_rect(cell, BACKGROUND);
    }

    fn new_line(&mut self) {
        self.column = 0;
        if self.row + 1 < self.rows {
            self.row += 1;
        } else {
            self.canvas.scroll_up(CELL, BACKGROUND);
        }
    }

    fn draw_cursor(&mut self, color: u32) {
        if self.column == self.columns {
            return;
        }
        let cell = self.cell();
        let bar = Rect::new(cell.x, cell.bottom() - SCALE * 2, CELL, SCALE * 2);
        self.canvas.fill_rect(bar, color);
    }
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        s.chars().for_each(|ch| self.put_char(ch));
        Ok(())
    }
}

static CONSOLE: Mutex<Option<Console>> = Mutex::new(None);
static DIRTY: AtomicBool = AtomicBool::new(false);

pub fn install(console: Console) {
    *CONSOLE.lock() = Some(console);
}

pub fn with_console<R>(f: impl FnOnce(&mut Console) -> R) -> Option<R> {
    let result = CONSOLE.lock().as_mut().map(f);
    DIRTY.store(true, Ordering::Release);
    result
}

pub fn peek<R>(f: impl FnOnce(&Console) -> R) -> Option<R> {
    CONSOLE.lock().as_ref().map(f)
}

pub fn take_dirty() -> bool {
    DIRTY.swap(false, Ordering::AcqRel)
}

pub fn force_unlock() {
    if CONSOLE.is_locked() {
        unsafe { CONSOLE.force_unlock() };
    }
}

pub fn print(args: fmt::Arguments) {
    force_unlock();
    if let Some(console) = CONSOLE.lock().as_mut() {
        let _ = console.write_fmt(args);
    }
    DIRTY.store(true, Ordering::Release);
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::console::print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
