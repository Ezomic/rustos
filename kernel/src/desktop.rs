use crate::console;
use crate::framebuffer::Display;
use crate::game;
use crate::gfx::{rgb, Canvas, Rect};
use crate::interrupts;
use crate::mouse::MouseEvent;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;
use spin::Mutex;

const TITLE_HEIGHT: i32 = 22;
const TASKBAR_HEIGHT: i32 = 30;
const BUTTON_WIDTH: i32 = 120;
const BUTTON_HEIGHT: i32 = 22;
const CLOCK_WIDTH: i32 = 48;

const DESKTOP: u32 = rgb(0x1d, 0x3b, 0x4a);
const TASKBAR: u32 = rgb(0x15, 0x18, 0x20);
const BUTTON: u32 = rgb(0x2a, 0x30, 0x3c);
const BUTTON_ACTIVE: u32 = rgb(0x3d, 0x5a, 0x80);
const BORDER: u32 = rgb(0x0a, 0x0b, 0x10);
const TITLE_ACTIVE: u32 = rgb(0x3d, 0x5a, 0x80);
const TITLE_INACTIVE: u32 = rgb(0x33, 0x38, 0x44);
const PANEL: u32 = rgb(0x1b, 0x1e, 0x28);
const TEXT: u32 = rgb(0xee, 0xee, 0xee);
const BLACK: u32 = rgb(0, 0, 0);
const WHITE: u32 = rgb(0xff, 0xff, 0xff);

const CURSOR_SPRITE: [&str; 17] = [
    "#",
    "##",
    "#.#",
    "#..#",
    "#...#",
    "#....#",
    "#.....#",
    "#......#",
    "#.......#",
    "#........#",
    "#.....#####",
    "#..#..#",
    "#.# #..#",
    "##  #..#",
    "#    #..#",
    "     #..#",
    "      ##",
];
const CURSOR_SIZE: (i32, i32) = (11, 17);

enum Content {
    Terminal,
    Game,
    Text(Vec<String>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Terminal,
    Game,
    Other,
}

struct Window {
    id: usize,
    title: &'static str,
    rect: Rect,
    content: Content,
}

impl Window {
    fn title_bar(&self) -> Rect {
        Rect::new(self.rect.x, self.rect.y, self.rect.w, TITLE_HEIGHT)
    }

    fn content_area(&self) -> Rect {
        Rect::new(
            self.rect.x + 1,
            self.rect.y + TITLE_HEIGHT,
            self.rect.w - 2,
            self.rect.h - TITLE_HEIGHT - 1,
        )
    }
}

pub struct Desktop {
    display: Display,
    canvas: Canvas,
    windows: Vec<Window>,
    cursor: (i32, i32),
    left_down: bool,
    grab: Option<(i32, i32)>,
    damage: Option<Rect>,
    clock_seconds: u64,
}

impl Desktop {
    pub fn new(
        display: Display,
        terminal_size: (i32, i32),
        game_size: (i32, i32),
        about: Vec<String>,
    ) -> Self {
        let canvas = Canvas::new(display.width(), display.height());
        let about_width = 280;
        let about_height = TITLE_HEIGHT + 1 + 16 + about.len() as i32 * 16;
        let margin = 20;
        let terminal = Window {
            id: 0,
            title: "Terminal",
            rect: Rect::new(
                margin,
                margin,
                terminal_size.0 + 2,
                terminal_size.1 + TITLE_HEIGHT + 1,
            ),
            content: Content::Terminal,
        };
        let column = terminal.rect.right() + margin;
        let breakout = Window {
            id: 2,
            title: "Breakout",
            rect: Rect::new(
                column,
                margin,
                game_size.0 + 2,
                game_size.1 + TITLE_HEIGHT + 1,
            ),
            content: Content::Game,
        };
        let about = Window {
            id: 1,
            title: "About",
            rect: Rect::new(
                column,
                breakout.rect.bottom() + margin,
                about_width,
                about_height,
            ),
            content: Content::Text(about),
        };
        let mut desktop = Self {
            cursor: (display.width() / 2, display.height() / 2),
            display,
            canvas,
            windows: alloc::vec![about, breakout, terminal],
            left_down: false,
            grab: None,
            damage: None,
            clock_seconds: 0,
        };
        desktop.damage_rect(desktop.screen());
        desktop
    }

    fn screen(&self) -> Rect {
        Rect::new(0, 0, self.display.width(), self.display.height())
    }

    fn taskbar_area(&self) -> Rect {
        Rect::new(
            0,
            self.display.height() - TASKBAR_HEIGHT,
            self.display.width(),
            TASKBAR_HEIGHT,
        )
    }

    fn clock_area(&self) -> Rect {
        let bar = self.taskbar_area();
        Rect::new(
            bar.right() - CLOCK_WIDTH - 8,
            bar.y + 4,
            CLOCK_WIDTH + 8,
            22,
        )
    }

    fn button_area(&self, index: usize) -> Rect {
        let bar = self.taskbar_area();
        Rect::new(
            4 + index as i32 * (BUTTON_WIDTH + 4),
            bar.y + 4,
            BUTTON_WIDTH,
            BUTTON_HEIGHT,
        )
    }

    fn cursor_area(&self) -> Rect {
        Rect::new(self.cursor.0, self.cursor.1, CURSOR_SIZE.0, CURSOR_SIZE.1)
    }

    fn damage_rect(&mut self, rect: Rect) {
        self.damage = Some(self.damage.map_or(rect, |d| d.union(&rect)));
    }

    fn raise(&mut self, index: usize) {
        let window = self.windows.remove(index);
        let rect = window.rect;
        self.windows.push(window);
        self.damage_rect(rect);
        self.damage_rect(self.taskbar_area());
    }

    fn window_at(&self, x: i32, y: i32) -> Option<usize> {
        self.windows.iter().rposition(|w| w.rect.contains(x, y))
    }

    fn press(&mut self, x: i32, y: i32) {
        if self.taskbar_area().contains(x, y) {
            let clicked = self
                .windows
                .iter()
                .position(|w| self.button_area(w.id).contains(x, y));
            if let Some(index) = clicked {
                self.raise(index);
            }
            return;
        }
        let Some(index) = self.window_at(x, y) else {
            return;
        };
        self.raise(index);
        let window = self.windows.last().expect("window just raised");
        if window.title_bar().contains(x, y) {
            self.grab = Some((x - window.rect.x, y - window.rect.y));
        }
    }

    fn drag_to(&mut self, x: i32, y: i32) {
        let Some((grab_x, grab_y)) = self.grab else {
            return;
        };
        let limit_y = self.taskbar_area().y - TITLE_HEIGHT;
        let max_x = self.display.width() - 40;
        let window = self.windows.last_mut().expect("dragging requires a window");
        let old = window.rect;
        window.rect.x = (x - grab_x).clamp(40 - window.rect.w, max_x);
        window.rect.y = (y - grab_y).clamp(0, limit_y);
        let new = window.rect;
        if new != old {
            self.damage_rect(old.union(&new));
        }
    }

    pub fn mouse_event(&mut self, event: &MouseEvent) {
        let old_cursor = self.cursor_area();
        let width = self.display.width();
        let height = self.display.height();
        self.cursor.0 = (self.cursor.0 + event.dx).clamp(0, width - 1);
        self.cursor.1 = (self.cursor.1 + event.dy).clamp(0, height - 1);
        let (x, y) = self.cursor;
        if event.left && !self.left_down {
            self.press(x, y);
        } else if event.left {
            self.drag_to(x, y);
        } else {
            self.grab = None;
        }
        self.left_down = event.left;
        let new_cursor = self.cursor_area();
        self.damage_rect(old_cursor.union(&new_cursor));
    }

    pub fn tick(&mut self, ticks: u64) {
        let seconds = ticks / interrupts::TICKS_PER_SECOND;
        if seconds != self.clock_seconds {
            self.clock_seconds = seconds;
            self.damage_rect(self.clock_area());
        }
    }

    pub fn focus(&self) -> Focus {
        match self.windows.last().map(|w| &w.content) {
            Some(Content::Terminal) => Focus::Terminal,
            Some(Content::Game) => Focus::Game,
            _ => Focus::Other,
        }
    }

    fn damage_content(&mut self, matches: fn(&Content) -> bool) {
        let area = self
            .windows
            .iter()
            .find(|w| matches(&w.content))
            .map(|w| w.content_area());
        if let Some(area) = area {
            self.damage_rect(area);
        }
    }

    fn damage_terminal(&mut self) {
        self.damage_content(|c| matches!(c, Content::Terminal));
    }

    fn damage_game(&mut self) {
        self.damage_content(|c| matches!(c, Content::Game));
    }

    pub fn redraw(&mut self) {
        let Some(damage) = self.damage.take() else {
            return;
        };
        self.canvas.set_clip(damage);
        self.compose();
        self.canvas.reset_clip();
        self.display.blit(&self.canvas, damage);
    }

    fn compose(&mut self) {
        let screen = self.screen();
        self.canvas.fill_rect(screen, DESKTOP);
        let last = self.windows.len().saturating_sub(1);
        for (index, window) in self.windows.iter().enumerate() {
            draw_window(&mut self.canvas, window, index == last);
        }
        self.draw_taskbar();
        self.draw_cursor();
    }

    fn draw_taskbar(&mut self) {
        let bar = self.taskbar_area();
        self.canvas.fill_rect(bar, TASKBAR);
        let last = self.windows.len().saturating_sub(1);
        for (index, window) in self.windows.iter().enumerate() {
            let area = self.button_area(window.id);
            let color = if index == last { BUTTON_ACTIVE } else { BUTTON };
            self.canvas.fill_rect(area, color);
            self.canvas
                .draw_text(area.x + 8, area.y + 7, window.title, TEXT, 1);
        }
        let mut clock = String::new();
        let _ = write!(
            clock,
            "{:02}:{:02}",
            self.clock_seconds / 60 % 100,
            self.clock_seconds % 60
        );
        let area = self.clock_area();
        self.canvas
            .draw_text(area.x + 4, area.y + 7, &clock, TEXT, 1);
    }

    fn draw_cursor(&mut self) {
        let (cursor_x, cursor_y) = self.cursor;
        for (row, line) in CURSOR_SPRITE.iter().enumerate() {
            for (column, cell) in line.chars().enumerate() {
                let color = match cell {
                    '#' => BLACK,
                    '.' => WHITE,
                    _ => continue,
                };
                self.canvas
                    .put_pixel(cursor_x + column as i32, cursor_y + row as i32, color);
            }
        }
    }
}

fn draw_window(canvas: &mut Canvas, window: &Window, focused: bool) {
    canvas.fill_rect(window.rect, BORDER);
    let title_color = if focused {
        TITLE_ACTIVE
    } else {
        TITLE_INACTIVE
    };
    let bar = window.title_bar();
    canvas.fill_rect(
        Rect::new(bar.x + 1, bar.y + 1, bar.w - 2, bar.h - 1),
        title_color,
    );
    canvas.draw_text(bar.x + 8, bar.y + 7, window.title, TEXT, 1);
    let area = window.content_area();
    canvas.fill_rect(area, PANEL);
    match &window.content {
        Content::Terminal => {
            console::peek(|c| canvas.blit(c.canvas(), area.x, area.y));
        }
        Content::Game => {
            game::peek(|g| canvas.blit(g.canvas(), area.x, area.y));
        }
        Content::Text(lines) => {
            for (index, line) in lines.iter().enumerate() {
                canvas.draw_text(area.x + 12, area.y + 12 + index as i32 * 16, line, TEXT, 1);
            }
        }
    }
}

static DESKTOP_STATE: Mutex<Option<Desktop>> = Mutex::new(None);

pub fn install(desktop: Desktop) {
    *DESKTOP_STATE.lock() = Some(desktop);
}

fn with<R>(f: impl FnOnce(&mut Desktop) -> R) -> Option<R> {
    DESKTOP_STATE.lock().as_mut().map(f)
}

pub fn screen_size() -> (i32, i32) {
    with(|d| (d.display.width(), d.display.height())).unwrap_or((0, 0))
}

pub fn focus() -> Focus {
    with(|d| d.focus()).unwrap_or(Focus::Other)
}

pub fn mouse_event(event: &MouseEvent) {
    with(|d| d.mouse_event(event));
}

pub fn tick(ticks: u64) {
    with(|d| d.tick(ticks));
}

pub fn redraw() {
    let terminal_changed = console::take_dirty();
    let game_changed = game::take_dirty();
    with(|d| {
        if terminal_changed {
            d.damage_terminal();
        }
        if game_changed {
            d.damage_game();
        }
        d.redraw();
    });
}

pub fn emergency_redraw() {
    if DESKTOP_STATE.is_locked() {
        unsafe { DESKTOP_STATE.force_unlock() };
    }
    console::force_unlock();
    with(|d| {
        d.damage_terminal();
        d.redraw();
    });
}
