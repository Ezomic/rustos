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

#[derive(Clone, Copy)]
struct Metrics {
    scale: i32,
    title: i32,
    border: i32,
    taskbar: i32,
    button_width: i32,
    button_height: i32,
    clock_width: i32,
    margin: i32,
    gap: i32,
}

impl Metrics {
    fn new(scale: i32) -> Self {
        Self {
            scale,
            title: 22 * scale,
            border: scale,
            taskbar: 30 * scale,
            button_width: 120 * scale,
            button_height: 22 * scale,
            clock_width: 48 * scale,
            margin: 20 * scale,
            gap: 4 * scale,
        }
    }
}

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
    fn title_bar(&self, m: &Metrics) -> Rect {
        Rect::new(self.rect.x, self.rect.y, self.rect.w, m.title)
    }

    fn content_area(&self, m: &Metrics) -> Rect {
        Rect::new(
            self.rect.x + m.border,
            self.rect.y + m.title,
            self.rect.w - 2 * m.border,
            self.rect.h - m.title - m.border,
        )
    }
}

pub struct Desktop {
    display: Display,
    canvas: Canvas,
    metrics: Metrics,
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
        scale: i32,
        terminal_size: (i32, i32),
        game_size: (i32, i32),
        about: Vec<String>,
    ) -> Self {
        let m = Metrics::new(scale);
        let canvas = Canvas::new(display.width(), display.height());
        let framed = |width: i32, height: i32| (width + 2 * m.border, height + m.title + m.border);
        let (terminal_width, terminal_height) = framed(terminal_size.0, terminal_size.1);
        let (game_width, game_height) = framed(game_size.0 * scale, game_size.1 * scale);
        let about_width = 280 * scale;
        let about_height = m.title + m.border + 16 * scale * (about.len() as i32 + 1);
        let terminal = Window {
            id: 0,
            title: "Terminal",
            rect: Rect::new(m.margin, m.margin, terminal_width, terminal_height),
            content: Content::Terminal,
        };
        let column = terminal.rect.right() + m.margin;
        let breakout = Window {
            id: 2,
            title: "Breakout",
            rect: Rect::new(column, m.margin, game_width, game_height),
            content: Content::Game,
        };
        let about = Window {
            id: 1,
            title: "About",
            rect: Rect::new(
                column,
                breakout.rect.bottom() + m.margin,
                about_width,
                about_height,
            ),
            content: Content::Text(about),
        };
        let mut desktop = Self {
            cursor: (display.width() / 2, display.height() / 2),
            display,
            canvas,
            metrics: m,
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
        let height = self.metrics.taskbar;
        Rect::new(
            0,
            self.display.height() - height,
            self.display.width(),
            height,
        )
    }

    fn clock_area(&self) -> Rect {
        let m = &self.metrics;
        let bar = self.taskbar_area();
        Rect::new(
            bar.right() - m.clock_width - 2 * m.gap,
            bar.y + m.gap,
            m.clock_width + 2 * m.gap,
            m.button_height,
        )
    }

    fn button_area(&self, index: usize) -> Rect {
        let m = &self.metrics;
        let bar = self.taskbar_area();
        Rect::new(
            m.gap + index as i32 * (m.button_width + m.gap),
            bar.y + m.gap,
            m.button_width,
            m.button_height,
        )
    }

    fn cursor_area(&self) -> Rect {
        let scale = self.metrics.scale;
        Rect::new(
            self.cursor.0,
            self.cursor.1,
            CURSOR_SIZE.0 * scale,
            CURSOR_SIZE.1 * scale,
        )
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
        if window.title_bar(&self.metrics).contains(x, y) {
            self.grab = Some((x - window.rect.x, y - window.rect.y));
        }
    }

    fn drag_to(&mut self, x: i32, y: i32) {
        let Some((grab_x, grab_y)) = self.grab else {
            return;
        };
        let keep_visible = 40 * self.metrics.scale;
        let limit_y = self.taskbar_area().y - self.metrics.title;
        let max_x = self.display.width() - keep_visible;
        let window = self.windows.last_mut().expect("dragging requires a window");
        let old = window.rect;
        window.rect.x = (x - grab_x).clamp(keep_visible - window.rect.w, max_x);
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
        let speed = self.metrics.scale;
        self.cursor.0 = (self.cursor.0 + event.dx * speed).clamp(0, width - 1);
        self.cursor.1 = (self.cursor.1 + event.dy * speed).clamp(0, height - 1);
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
            .map(|w| w.content_area(&self.metrics));
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
            draw_window(&mut self.canvas, window, index == last, &self.metrics);
        }
        self.draw_taskbar();
        self.draw_cursor();
    }

    fn draw_taskbar(&mut self) {
        let scale = self.metrics.scale;
        let bar = self.taskbar_area();
        self.canvas.fill_rect(bar, TASKBAR);
        let last = self.windows.len().saturating_sub(1);
        for (index, window) in self.windows.iter().enumerate() {
            let area = self.button_area(window.id);
            let color = if index == last { BUTTON_ACTIVE } else { BUTTON };
            self.canvas.fill_rect(area, color);
            self.canvas.draw_text(
                area.x + 8 * scale,
                area.y + 7 * scale,
                window.title,
                TEXT,
                scale,
            );
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
            .draw_text(area.x + 4 * scale, area.y + 7 * scale, &clock, TEXT, scale);
    }

    fn draw_cursor(&mut self) {
        let scale = self.metrics.scale;
        let (cursor_x, cursor_y) = self.cursor;
        for (row, line) in CURSOR_SPRITE.iter().enumerate() {
            for (column, cell) in line.chars().enumerate() {
                let color = match cell {
                    '#' => BLACK,
                    '.' => WHITE,
                    _ => continue,
                };
                let pixel = Rect::new(
                    cursor_x + column as i32 * scale,
                    cursor_y + row as i32 * scale,
                    scale,
                    scale,
                );
                self.canvas.fill_rect(pixel, color);
            }
        }
    }
}

fn draw_window(canvas: &mut Canvas, window: &Window, focused: bool, m: &Metrics) {
    canvas.fill_rect(window.rect, BORDER);
    let title_color = if focused {
        TITLE_ACTIVE
    } else {
        TITLE_INACTIVE
    };
    let bar = window.title_bar(m);
    canvas.fill_rect(
        Rect::new(
            bar.x + m.border,
            bar.y + m.border,
            bar.w - 2 * m.border,
            bar.h - m.border,
        ),
        title_color,
    );
    canvas.draw_text(
        bar.x + 8 * m.scale,
        bar.y + 7 * m.scale,
        window.title,
        TEXT,
        m.scale,
    );
    let area = window.content_area(m);
    canvas.fill_rect(area, PANEL);
    match &window.content {
        Content::Terminal => {
            console::peek(|c| canvas.blit(c.canvas(), area.x, area.y));
        }
        Content::Game => {
            game::peek(|g| canvas.blit_scaled(g.canvas(), area.x, area.y, m.scale));
        }
        Content::Text(lines) => {
            for (index, line) in lines.iter().enumerate() {
                let y = area.y + (12 + index as i32 * 16) * m.scale;
                canvas.draw_text(area.x + 12 * m.scale, y, line, TEXT, m.scale);
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

pub fn display_summary() -> String {
    with(|d| d.display.summary()).unwrap_or_default()
}

pub fn screen_size() -> (i32, i32) {
    with(|d| (d.display.width(), d.display.height())).unwrap_or((0, 0))
}

pub fn ui_scale() -> i32 {
    with(|d| d.metrics.scale).unwrap_or(1)
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
