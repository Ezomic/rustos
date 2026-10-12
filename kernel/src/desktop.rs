use crate::console;
use crate::framebuffer::Display;
use crate::game;
use crate::gfx::{rgb, Canvas, Rect};
use crate::interrupts;
use crate::memory;
use crate::mouse::MouseEvent;
use crate::widgets::{
    draw_button, draw_centered_text, draw_label, draw_progress, BORDER, BUTTON, BUTTON_ACTIVE,
    BUTTON_HOVER, TEXT,
};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;
use spin::Mutex;

const DESKTOP: u32 = rgb(0x1d, 0x3b, 0x4a);
const TASKBAR: u32 = rgb(0x15, 0x18, 0x20);
const TITLE_ACTIVE: u32 = rgb(0x3d, 0x5a, 0x80);
const TITLE_INACTIVE: u32 = rgb(0x33, 0x38, 0x44);
const PANEL: u32 = rgb(0x1b, 0x1e, 0x28);
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
    start_width: i32,
    clock_width: i32,
    control: i32,
    menu_width: i32,
    menu_item: i32,
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
            start_width: 70 * scale,
            clock_width: 48 * scale,
            control: 16 * scale,
            menu_width: 160 * scale,
            menu_item: 24 * scale,
            margin: 20 * scale,
            gap: 4 * scale,
        }
    }
}

enum Content {
    Terminal,
    Game,
    System,
    Text(Vec<String>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Terminal,
    Game,
    Other,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Open,
    Minimized,
    Closed,
}

#[derive(Clone, Copy)]
enum Action {
    RestartGame,
    ClearTerminal,
}

struct Window {
    id: usize,
    title: &'static str,
    rect: Rect,
    content: Content,
    state: State,
}

impl Window {
    fn visible(&self) -> bool {
        self.state == State::Open
    }

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

    fn controls(&self, m: &Metrics) -> (Rect, Rect) {
        let y = self.rect.y + (m.title - m.control) / 2;
        let close = Rect::new(
            self.rect.right() - m.border - m.gap - m.control,
            y,
            m.control,
            m.control,
        );
        let minimize = Rect::new(close.x - m.gap - m.control, y, m.control, m.control);
        (minimize, close)
    }
}

fn system_buttons(area: Rect, m: &Metrics) -> [(Rect, &'static str, Action); 2] {
    let s = m.scale;
    let top = area.y + 104 * s;
    [
        (
            Rect::new(area.x + 12 * s, top, 130 * s, 24 * s),
            "Restart game",
            Action::RestartGame,
        ),
        (
            Rect::new(area.x + 152 * s, top, 130 * s, 24 * s),
            "Clear terminal",
            Action::ClearTerminal,
        ),
    ]
}

struct Frame {
    m: Metrics,
    cursor: (i32, i32),
    down: bool,
    screen: (i32, i32),
}

pub struct Desktop {
    display: Display,
    canvas: Canvas,
    metrics: Metrics,
    windows: Vec<Window>,
    cursor: (i32, i32),
    left_down: bool,
    grab: Option<(i32, i32)>,
    menu_open: bool,
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
        let (system_width, system_height) = framed(300 * scale, 140 * scale);
        let about_width = 280 * scale;
        let about_height = m.title + m.border + 16 * scale * (about.len() as i32 + 1);
        let terminal = Window {
            id: 0,
            title: "Terminal",
            rect: Rect::new(m.margin, m.margin, terminal_width, terminal_height),
            content: Content::Terminal,
            state: State::Open,
        };
        let column = terminal.rect.right() + m.margin;
        let breakout = Window {
            id: 1,
            title: "Breakout",
            rect: Rect::new(column, m.margin, game_width, game_height),
            content: Content::Game,
            state: State::Open,
        };
        let system = Window {
            id: 2,
            title: "System",
            rect: Rect::new(m.margin * 3, m.margin * 4, system_width, system_height),
            content: Content::System,
            state: State::Closed,
        };
        let about = Window {
            id: 3,
            title: "About",
            rect: Rect::new(
                column,
                breakout.rect.bottom() + m.margin,
                about_width,
                about_height,
            ),
            content: Content::Text(about),
            state: State::Open,
        };
        let mut desktop = Self {
            cursor: (display.width() / 2, display.height() / 2),
            display,
            canvas,
            metrics: m,
            windows: alloc::vec![about, breakout, system, terminal],
            left_down: false,
            grab: None,
            menu_open: false,
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

    fn start_area(&self) -> Rect {
        let m = &self.metrics;
        let bar = self.taskbar_area();
        Rect::new(m.gap, bar.y + m.gap, m.start_width, m.button_height)
    }

    fn button_area(&self, slot: usize) -> Rect {
        let m = &self.metrics;
        let bar = self.taskbar_area();
        let first = m.gap + m.start_width + m.gap;
        Rect::new(
            first + slot as i32 * (m.button_width + m.gap),
            bar.y + m.gap,
            m.button_width,
            m.button_height,
        )
    }

    fn taskbar_ids(&self) -> Vec<usize> {
        let mut ids: Vec<usize> = self
            .windows
            .iter()
            .filter(|w| w.state != State::Closed)
            .map(|w| w.id)
            .collect();
        ids.sort_unstable();
        ids
    }

    fn app_ids(&self) -> Vec<usize> {
        let mut ids: Vec<usize> = self.windows.iter().map(|w| w.id).collect();
        ids.sort_unstable();
        ids
    }

    fn menu_area(&self) -> Rect {
        let m = &self.metrics;
        let height = self.windows.len() as i32 * m.menu_item + 2 * m.border;
        Rect::new(0, self.taskbar_area().y - height, m.menu_width, height)
    }

    fn menu_item_area(&self, index: usize) -> Rect {
        let m = &self.metrics;
        let menu = self.menu_area();
        Rect::new(
            menu.x + m.border,
            menu.y + m.border + index as i32 * m.menu_item,
            menu.w - 2 * m.border,
            m.menu_item,
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

    fn window_index(&self, id: usize) -> Option<usize> {
        self.windows.iter().position(|w| w.id == id)
    }

    fn top_visible(&self) -> Option<usize> {
        self.windows.iter().rposition(|w| w.visible())
    }

    fn window_at(&self, x: i32, y: i32) -> Option<usize> {
        self.windows
            .iter()
            .rposition(|w| w.visible() && w.rect.contains(x, y))
    }

    fn raise(&mut self, index: usize) {
        let window = self.windows.remove(index);
        let rect = window.rect;
        self.windows.push(window);
        self.damage_rect(rect);
        self.damage_rect(self.taskbar_area());
    }

    fn set_state(&mut self, index: usize, state: State) {
        self.windows[index].state = state;
        let rect = self.windows[index].rect;
        self.damage_rect(rect);
        self.damage_rect(self.taskbar_area());
    }

    fn open_window(&mut self, id: usize) {
        if let Some(index) = self.window_index(id) {
            self.set_state(index, State::Open);
            self.raise(index);
        }
    }

    fn toggle_menu(&mut self) {
        self.menu_open = !self.menu_open;
        self.damage_rect(self.menu_area());
        self.damage_rect(self.start_area());
    }

    fn close_menu(&mut self) {
        if self.menu_open {
            self.toggle_menu();
        }
    }

    fn system_area(&self) -> Option<Rect> {
        self.windows
            .iter()
            .find(|w| w.visible() && matches!(w.content, Content::System))
            .map(|w| w.content_area(&self.metrics))
    }

    fn press(&mut self, x: i32, y: i32) {
        if self.start_area().contains(x, y) {
            self.toggle_menu();
            return;
        }
        if self.menu_open {
            let ids = self.app_ids();
            let chosen = (0..ids.len()).find(|&i| self.menu_item_area(i).contains(x, y));
            let inside = self.menu_area().contains(x, y);
            self.close_menu();
            if let Some(slot) = chosen {
                self.open_window(ids[slot]);
            }
            if inside {
                return;
            }
        }
        if self.taskbar_area().contains(x, y) {
            let ids = self.taskbar_ids();
            let clicked = (0..ids.len()).find(|&slot| self.button_area(slot).contains(x, y));
            if let Some(slot) = clicked {
                self.taskbar_click(ids[slot]);
            }
            return;
        }
        let Some(index) = self.window_at(x, y) else {
            return;
        };
        self.raise(index);
        let last = self.windows.len() - 1;
        let m = self.metrics;
        let window = &self.windows[last];
        let (minimize, close) = window.controls(&m);
        let in_title = window.title_bar(&m).contains(x, y);
        let origin = (window.rect.x, window.rect.y);
        let area = window.content_area(&m);
        let is_system = matches!(window.content, Content::System);
        if close.contains(x, y) {
            self.set_state(last, State::Closed);
        } else if minimize.contains(x, y) {
            self.set_state(last, State::Minimized);
        } else if in_title {
            self.grab = Some((x - origin.0, y - origin.1));
        } else if is_system {
            self.press_system(area, x, y);
        }
    }

    fn press_system(&mut self, area: Rect, x: i32, y: i32) {
        let action = system_buttons(area, &self.metrics)
            .into_iter()
            .find(|(rect, _, _)| rect.contains(x, y))
            .map(|(_, _, action)| action);
        match action {
            Some(Action::RestartGame) => game::restart(),
            Some(Action::ClearTerminal) => {
                console::with_console(|c| c.clear());
            }
            None => {}
        }
    }

    fn taskbar_click(&mut self, id: usize) {
        let Some(index) = self.window_index(id) else {
            return;
        };
        match self.windows[index].state {
            State::Minimized => self.open_window(id),
            State::Open if self.top_visible() == Some(index) => {
                self.set_state(index, State::Minimized)
            }
            State::Open => self.raise(index),
            State::Closed => {}
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
        let old_position = self.cursor;
        let width = self.display.width();
        let height = self.display.height();
        let speed = self.metrics.scale;
        self.cursor.0 = (self.cursor.0 + event.dx * speed).clamp(0, width - 1);
        self.cursor.1 = (self.cursor.1 + event.dy * speed).clamp(0, height - 1);
        let (x, y) = self.cursor;
        let button_changed = event.left != self.left_down;
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
        if self.menu_open {
            self.damage_rect(self.menu_area());
        }
        if let Some(area) = self.system_area() {
            let hovered = area.contains(old_position.0, old_position.1) || area.contains(x, y);
            if hovered || button_changed {
                self.damage_rect(area);
            }
        }
    }

    pub fn tick(&mut self, ticks: u64) {
        let seconds = ticks / interrupts::TICKS_PER_SECOND;
        if seconds != self.clock_seconds {
            self.clock_seconds = seconds;
            self.damage_rect(self.clock_area());
            if let Some(area) = self.system_area() {
                self.damage_rect(area);
            }
        }
    }

    pub fn focus(&self) -> Focus {
        let top = self.top_visible().map(|i| &self.windows[i].content);
        match top {
            Some(Content::Terminal) => Focus::Terminal,
            Some(Content::Game) => Focus::Game,
            _ => Focus::Other,
        }
    }

    fn damage_content(&mut self, matches: fn(&Content) -> bool) {
        let area = self
            .windows
            .iter()
            .find(|w| w.visible() && matches(&w.content))
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
        let frame = Frame {
            m: self.metrics,
            cursor: self.cursor,
            down: self.left_down,
            screen: (self.display.width(), self.display.height()),
        };
        let top = self.top_visible();
        for (index, window) in self.windows.iter().enumerate() {
            if window.visible() {
                draw_window(&mut self.canvas, window, Some(index) == top, &frame);
            }
        }
        self.draw_taskbar();
        if self.menu_open {
            self.draw_menu();
        }
        self.draw_cursor();
    }

    fn draw_taskbar(&mut self) {
        let scale = self.metrics.scale;
        let bar = self.taskbar_area();
        self.canvas.fill_rect(bar, TASKBAR);
        let start = self.start_area();
        let start_color = if self.menu_open {
            BUTTON_ACTIVE
        } else {
            BUTTON
        };
        self.canvas.fill_rect(start, start_color);
        draw_centered_text(&mut self.canvas, start, "Start", scale);
        let top = self.top_visible().map(|i| self.windows[i].id);
        for (slot, id) in self.taskbar_ids().into_iter().enumerate() {
            let Some(window) = self.window_index(id).map(|i| &self.windows[i]) else {
                continue;
            };
            let area = self.button_area(slot);
            let color = if Some(id) == top {
                BUTTON_ACTIVE
            } else {
                BUTTON
            };
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

    fn draw_menu(&mut self) {
        let scale = self.metrics.scale;
        let menu = self.menu_area();
        self.canvas.fill_rect(menu, BORDER);
        for (index, id) in self.app_ids().into_iter().enumerate() {
            let Some(window) = self.window_index(id).map(|i| &self.windows[i]) else {
                continue;
            };
            let item = self.menu_item_area(index);
            let hovered = item.contains(self.cursor.0, self.cursor.1);
            let color = if hovered { BUTTON_HOVER } else { BUTTON };
            self.canvas.fill_rect(item, color);
            self.canvas.draw_text(
                item.x + 8 * scale,
                item.y + (item.h - 8 * scale) / 2,
                window.title,
                TEXT,
                scale,
            );
        }
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

fn draw_window(canvas: &mut Canvas, window: &Window, focused: bool, frame: &Frame) {
    let m = &frame.m;
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
    draw_label(
        canvas,
        bar.x + 8 * m.scale,
        bar.y + 7 * m.scale,
        window.title,
        m.scale,
    );
    let (minimize, close) = window.controls(m);
    for (rect, glyph) in [(minimize, "_"), (close, "x")] {
        canvas.fill_rect(rect, BUTTON);
        draw_centered_text(canvas, rect, glyph, m.scale);
    }
    let area = window.content_area(m);
    canvas.fill_rect(area, PANEL);
    match &window.content {
        Content::Terminal => {
            console::peek(|c| canvas.blit(c.canvas(), area.x, area.y));
        }
        Content::Game => {
            game::peek(|g| canvas.blit_scaled(g.canvas(), area.x, area.y, m.scale));
        }
        Content::System => draw_system(canvas, area, frame),
        Content::Text(lines) => {
            for (index, line) in lines.iter().enumerate() {
                let y = area.y + (12 + index as i32 * 16) * m.scale;
                draw_label(canvas, area.x + 12 * m.scale, y, line, m.scale);
            }
        }
    }
}

fn draw_system(canvas: &mut Canvas, area: Rect, frame: &Frame) {
    let s = frame.m.scale;
    let seconds = interrupts::ticks() / interrupts::TICKS_PER_SECOND;
    let used = memory::heap_used();
    let total = memory::HEAP_SIZE;
    let tenths = used * 10 >> 20;
    let lines = [
        (
            12,
            format!("Uptime {:02}:{:02}", seconds / 60 % 100, seconds % 60),
        ),
        (
            32,
            format!(
                "Heap {}.{} of {} MiB",
                tenths / 10,
                tenths % 10,
                total >> 20
            ),
        ),
        (
            68,
            format!("Screen {}x{} scale {}x", frame.screen.0, frame.screen.1, s),
        ),
    ];
    for (y, text) in &lines {
        draw_label(canvas, area.x + 12 * s, area.y + y * s, text, s);
    }
    let bar = Rect::new(area.x + 12 * s, area.y + 46 * s, 276 * s, 12 * s);
    draw_progress(canvas, bar, (used * 1000 / total) as i32, s);
    for (rect, label, _) in system_buttons(area, &frame.m) {
        let hovered = rect.contains(frame.cursor.0, frame.cursor.1);
        draw_button(canvas, rect, label, hovered, hovered && frame.down, s);
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
