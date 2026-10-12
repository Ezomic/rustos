use core::sync::atomic::{AtomicU64, Ordering};
use crossbeam_queue::ArrayQueue;
use pc_keyboard::{
    layouts::Us104Key, DecodedKey, HandleControl, KeyCode, KeyState, PS2Keyboard, ScancodeSet1,
};
use spin::Once;

const QUEUE_CAPACITY: usize = 128;

static SCANCODES: Once<ArrayQueue<u8>> = Once::new();
static SEEN: AtomicU64 = AtomicU64::new(0);

pub fn init_queue() {
    SCANCODES.call_once(|| ArrayQueue::new(QUEUE_CAPACITY));
}

pub fn scancodes_seen() -> u64 {
    SEEN.load(Ordering::Relaxed)
}

pub fn push_scancode(scancode: u8) {
    SEEN.fetch_add(1, Ordering::Relaxed);
    if let Some(queue) = SCANCODES.get() {
        let _ = queue.push(scancode);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Left,
    Right,
}

pub struct KeyInput {
    pub key: Key,
    pub pressed: bool,
}

pub struct Keyboard {
    decoder: PS2Keyboard<Us104Key, ScancodeSet1>,
}

impl Keyboard {
    pub const fn new() -> Self {
        Self {
            decoder: PS2Keyboard::new(ScancodeSet1::new(), Us104Key, HandleControl::Ignore),
        }
    }

    pub fn next_input(&mut self) -> Option<KeyInput> {
        let queue = SCANCODES.get()?;
        while let Some(byte) = queue.pop() {
            let Ok(Some(event)) = self.decoder.add_byte(byte) else {
                continue;
            };
            let arrow = match event.code {
                KeyCode::ArrowLeft => Some(Key::Left),
                KeyCode::ArrowRight => Some(Key::Right),
                _ => None,
            };
            if let Some(key) = arrow {
                return Some(KeyInput {
                    key,
                    pressed: event.state != KeyState::Up,
                });
            }
            if let Some(DecodedKey::Unicode(ch)) = self.decoder.process_keyevent(event) {
                return Some(KeyInput {
                    key: Key::Char(ch),
                    pressed: true,
                });
            }
        }
        None
    }
}
