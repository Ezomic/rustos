use core::arch::asm;
use pc_keyboard::{layouts::Us104Key, DecodedKey, HandleControl, PS2Keyboard, ScancodeSet1};

const DATA_PORT: u16 = 0x60;
const STATUS_PORT: u16 = 0x64;
const OUTPUT_FULL: u8 = 1;
const FROM_MOUSE: u8 = 1 << 5;

pub struct Keyboard {
    decoder: PS2Keyboard<Us104Key, ScancodeSet1>,
}

impl Keyboard {
    pub const fn new() -> Self {
        Self {
            decoder: PS2Keyboard::new(ScancodeSet1::new(), Us104Key, HandleControl::Ignore),
        }
    }

    pub fn poll(&mut self) -> Option<char> {
        let status = unsafe { port_in(STATUS_PORT) };
        if status & OUTPUT_FULL == 0 {
            return None;
        }
        let byte = unsafe { port_in(DATA_PORT) };
        if status & FROM_MOUSE != 0 {
            return None;
        }
        let event = self.decoder.add_byte(byte).ok()??;
        match self.decoder.process_keyevent(event)? {
            DecodedKey::Unicode(ch) => Some(ch),
            DecodedKey::RawKey(_) => None,
        }
    }
}

unsafe fn port_in(port: u16) -> u8 {
    let value: u8;
    asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags));
    value
}
