use crossbeam_queue::ArrayQueue;
use pc_keyboard::{layouts::Us104Key, DecodedKey, HandleControl, PS2Keyboard, ScancodeSet1};
use spin::Once;

const QUEUE_CAPACITY: usize = 128;

static SCANCODES: Once<ArrayQueue<u8>> = Once::new();

pub fn init_queue() {
    SCANCODES.call_once(|| ArrayQueue::new(QUEUE_CAPACITY));
}

pub fn push_scancode(scancode: u8) {
    if let Some(queue) = SCANCODES.get() {
        let _ = queue.push(scancode);
    }
}

pub fn has_input() -> bool {
    SCANCODES.get().is_some_and(|queue| !queue.is_empty())
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

    pub fn next_char(&mut self) -> Option<char> {
        let queue = SCANCODES.get()?;
        while let Some(byte) = queue.pop() {
            let Ok(Some(event)) = self.decoder.add_byte(byte) else {
                continue;
            };
            if let Some(DecodedKey::Unicode(ch)) = self.decoder.process_keyevent(event) {
                return Some(ch);
            }
        }
        None
    }
}
