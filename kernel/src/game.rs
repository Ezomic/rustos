use crate::gfx::{rgb, Canvas, Rect, Sprite};
use crate::keyboard::Key;
use alloc::format;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;

pub const WIDTH: i32 = 480;
pub const HEIGHT: i32 = 360;
const STEP_TICKS: u64 = 2;
const MAX_CATCH_UP: u32 = 5;
const FIXED: i32 = 256;

const COLUMNS: usize = 10;
const ROWS: usize = 6;
const BRICK_WIDTH: i32 = 44;
const BRICK_HEIGHT: i32 = 14;
const BRICK_GAP: i32 = 2;
const BRICKS_LEFT: i32 = (WIDTH - COLUMNS as i32 * (BRICK_WIDTH + BRICK_GAP) + BRICK_GAP) / 2;
const BRICKS_TOP: i32 = 44;

const PADDLE_WIDTH: i32 = 64;
const PADDLE_HEIGHT: i32 = 10;
const PADDLE_Y: i32 = HEIGHT - 24;
const PADDLE_SPEED: i32 = 6;
const BALL_SIZE: i32 = 8;
const BALL_LAUNCH_VX: i32 = 384;
const BALL_SPEED_Y: i32 = 768;
const STARTING_LIVES: u32 = 3;

const BACKGROUND: u32 = rgb(0x0c, 0x0e, 0x18);
const TEXT: u32 = rgb(0xee, 0xee, 0xee);
const ROW_COLORS: [u32; ROWS] = [
    rgb(0xe0, 0x4a, 0x4a),
    rgb(0xe0, 0x8a, 0x3a),
    rgb(0xe0, 0xc8, 0x3a),
    rgb(0x4a, 0xc0, 0x5a),
    rgb(0x3a, 0x9a, 0xe0),
    rgb(0x8a, 0x5a, 0xe0),
];
const HEART_ART: [&str; 7] = [
    ".##.##.", "#######", "#######", "#######", ".#####.", "..###..", "...#...",
];

static DIRTY: AtomicBool = AtomicBool::new(false);
static GAME: Mutex<Option<Breakout>> = Mutex::new(None);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Waiting,
    Playing,
    Lost,
    Won,
}

struct Sprites {
    bricks: [Sprite; ROWS],
    paddle: Sprite,
    ball: Sprite,
    heart: Sprite,
}

pub struct Breakout {
    canvas: Canvas,
    sprites: Sprites,
    bricks: [[bool; COLUMNS]; ROWS],
    phase: Phase,
    paddle_x: i32,
    ball_x: i32,
    ball_y: i32,
    velocity_x: i32,
    velocity_y: i32,
    score: u32,
    lives: u32,
    left: bool,
    right: bool,
    action: bool,
    last_step: u64,
    launches: u32,
}

impl Breakout {
    pub fn new(now: u64) -> Self {
        let sprites = Sprites {
            bricks: ROW_COLORS.map(|color| Sprite::bevel_box(BRICK_WIDTH, BRICK_HEIGHT, color)),
            paddle: Sprite::bevel_box(PADDLE_WIDTH, PADDLE_HEIGHT, rgb(0xb0, 0xb8, 0xc8)),
            ball: Sprite::disc(BALL_SIZE, rgb(0xff, 0xff, 0xff)),
            heart: Sprite::from_art(&HEART_ART, &[('#', rgb(0xe0, 0x4a, 0x4a))]),
        };
        let mut game = Self {
            canvas: Canvas::new(WIDTH, HEIGHT),
            sprites,
            bricks: [[true; COLUMNS]; ROWS],
            phase: Phase::Waiting,
            paddle_x: (WIDTH - PADDLE_WIDTH) / 2,
            ball_x: 0,
            ball_y: 0,
            velocity_x: 0,
            velocity_y: 0,
            score: 0,
            lives: STARTING_LIVES,
            left: false,
            right: false,
            action: false,
            last_step: now,
            launches: 0,
        };
        game.rest_ball();
        game.render();
        game
    }

    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    pub fn key(&mut self, key: Key, pressed: bool) {
        match key {
            Key::Left => self.left = pressed,
            Key::Right => self.right = pressed,
            Key::Char(' ') => self.action |= pressed,
            Key::Char(_) => {}
        }
    }

    pub fn pause(&mut self, now: u64) {
        self.left = false;
        self.right = false;
        self.last_step = now;
    }

    pub fn update(&mut self, now: u64) {
        let mut steps = 0;
        while now - self.last_step >= STEP_TICKS && steps < MAX_CATCH_UP {
            self.step();
            self.last_step += STEP_TICKS;
            steps += 1;
        }
        if now - self.last_step >= STEP_TICKS {
            self.last_step = now;
        }
        if steps > 0 {
            self.render();
        }
    }

    fn rest_ball(&mut self) {
        self.ball_x = (self.paddle_x + (PADDLE_WIDTH - BALL_SIZE) / 2) * FIXED;
        self.ball_y = (PADDLE_Y - BALL_SIZE) * FIXED;
    }

    fn reset(&mut self) {
        self.bricks = [[true; COLUMNS]; ROWS];
        self.score = 0;
        self.lives = STARTING_LIVES;
        self.phase = Phase::Waiting;
    }

    fn step(&mut self) {
        let direction = self.right as i32 - self.left as i32;
        self.paddle_x = (self.paddle_x + direction * PADDLE_SPEED).clamp(0, WIDTH - PADDLE_WIDTH);
        let action = core::mem::take(&mut self.action);
        match self.phase {
            Phase::Waiting => {
                self.rest_ball();
                if action {
                    self.launches += 1;
                    let side = if self.launches % 2 == 0 { 1 } else { -1 };
                    self.velocity_x = side * BALL_LAUNCH_VX;
                    self.velocity_y = -BALL_SPEED_Y;
                    self.phase = Phase::Playing;
                }
            }
            Phase::Playing => self.step_ball(),
            Phase::Lost | Phase::Won => {
                if action {
                    self.reset();
                }
            }
        }
    }

    fn step_ball(&mut self) {
        self.ball_x += self.velocity_x;
        self.ball_y += self.velocity_y;
        if self.ball_x < 0 {
            self.ball_x = 0;
            self.velocity_x = -self.velocity_x;
        }
        let max_x = (WIDTH - BALL_SIZE) * FIXED;
        if self.ball_x > max_x {
            self.ball_x = max_x;
            self.velocity_x = -self.velocity_x;
        }
        if self.ball_y < 0 {
            self.ball_y = 0;
            self.velocity_y = -self.velocity_y;
        }
        self.bounce_off_paddle();
        self.break_brick();
        if self.ball_y > HEIGHT * FIXED {
            self.lives -= 1;
            self.phase = if self.lives == 0 {
                Phase::Lost
            } else {
                Phase::Waiting
            };
        }
    }

    fn ball_rect(&self) -> Rect {
        Rect::new(
            self.ball_x / FIXED,
            self.ball_y / FIXED,
            BALL_SIZE,
            BALL_SIZE,
        )
    }

    fn bounce_off_paddle(&mut self) {
        let paddle = Rect::new(self.paddle_x, PADDLE_Y, PADDLE_WIDTH, PADDLE_HEIGHT);
        let ball = self.ball_rect();
        if self.velocity_y <= 0 || ball.intersect(&paddle).is_none() {
            return;
        }
        let offset = ball.x + BALL_SIZE / 2 - (paddle.x + PADDLE_WIDTH / 2);
        self.velocity_x = offset * 1024 / (PADDLE_WIDTH / 2 + BALL_SIZE / 2);
        self.velocity_y = -BALL_SPEED_Y;
        self.ball_y = (PADDLE_Y - BALL_SIZE) * FIXED;
    }

    fn break_brick(&mut self) {
        let ball = self.ball_rect();
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                if !self.bricks[row][column] {
                    continue;
                }
                let Some(overlap) = ball.intersect(&brick_rect(row, column)) else {
                    continue;
                };
                self.bricks[row][column] = false;
                self.score += 10;
                if overlap.w < overlap.h {
                    self.velocity_x = -self.velocity_x;
                } else {
                    self.velocity_y = -self.velocity_y;
                }
                if self.bricks.iter().flatten().all(|alive| !alive) {
                    self.phase = Phase::Won;
                }
                return;
            }
        }
    }

    fn render(&mut self) {
        let bounds = Rect::new(0, 0, WIDTH, HEIGHT);
        self.canvas.fill_rect(bounds, BACKGROUND);
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                if self.bricks[row][column] {
                    let rect = brick_rect(row, column);
                    self.canvas
                        .draw_sprite(&self.sprites.bricks[row], rect.x, rect.y);
                }
            }
        }
        self.canvas
            .draw_sprite(&self.sprites.paddle, self.paddle_x, PADDLE_Y);
        let ball = self.ball_rect();
        if self.phase != Phase::Lost {
            self.canvas.draw_sprite(&self.sprites.ball, ball.x, ball.y);
        }
        self.canvas
            .draw_text(8, 8, &format!("SCORE {}", self.score), TEXT, 2);
        for life in 0..self.lives as i32 {
            let x = WIDTH - 8 - (life + 1) * (self.sprites.heart.width() + 4);
            self.canvas.draw_sprite(&self.sprites.heart, x, 10);
        }
        let message = match self.phase {
            Phase::Waiting => Some("PRESS SPACE"),
            Phase::Lost => Some("GAME OVER"),
            Phase::Won => Some("YOU WIN"),
            Phase::Playing => None,
        };
        if let Some(message) = message {
            let x = (WIDTH - message.len() as i32 * 16) / 2;
            self.canvas.draw_text(x, HEIGHT / 2, message, TEXT, 2);
        }
        DIRTY.store(true, Ordering::Release);
    }
}

fn brick_rect(row: usize, column: usize) -> Rect {
    Rect::new(
        BRICKS_LEFT + column as i32 * (BRICK_WIDTH + BRICK_GAP),
        BRICKS_TOP + row as i32 * (BRICK_HEIGHT + BRICK_GAP),
        BRICK_WIDTH,
        BRICK_HEIGHT,
    )
}

pub fn install(game: Breakout) {
    *GAME.lock() = Some(game);
}

pub fn peek<R>(f: impl FnOnce(&Breakout) -> R) -> Option<R> {
    GAME.lock().as_ref().map(f)
}

pub fn key(key: Key, pressed: bool) {
    if let Some(game) = GAME.lock().as_mut() {
        game.key(key, pressed);
    }
}

pub fn pause(now: u64) {
    if let Some(game) = GAME.lock().as_mut() {
        game.pause(now);
    }
}

pub fn update(now: u64) {
    if let Some(game) = GAME.lock().as_mut() {
        game.update(now);
    }
}

pub fn take_dirty() -> bool {
    DIRTY.swap(false, Ordering::AcqRel)
}
