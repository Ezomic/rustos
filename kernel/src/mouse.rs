use crossbeam_queue::ArrayQueue;
use spin::Once;
use x86_64::instructions::port::Port;

const QUEUE_CAPACITY: usize = 256;
const SPIN_LIMIT: u32 = 200_000;
const ACK: u8 = 0xfa;

static BYTES: Once<ArrayQueue<u8>> = Once::new();

pub struct MouseEvent {
    pub dx: i32,
    pub dy: i32,
    pub left: bool,
}

pub fn init() -> bool {
    BYTES.call_once(|| ArrayQueue::new(QUEUE_CAPACITY));
    let mut data = Port::<u8>::new(0x60);
    let mut command = Port::<u8>::new(0x64);
    unsafe {
        while command.read() & 1 != 0 {
            data.read();
        }
        if !wait_input_clear(&mut command) {
            return false;
        }
        command.write(0xa8);
        if !wait_input_clear(&mut command) {
            return false;
        }
        command.write(0x20);
        if !wait_output_full(&mut command) {
            return false;
        }
        let config = (data.read() | 0x02) & !0x20;
        if !wait_input_clear(&mut command) {
            return false;
        }
        command.write(0x60);
        if !wait_input_clear(&mut command) {
            return false;
        }
        data.write(config);
        let enabled = send(&mut command, &mut data, 0xf6) && send(&mut command, &mut data, 0xf4);
        for _ in 0..SPIN_LIMIT {
            if command.read() & 1 == 0 {
                break;
            }
            data.read();
        }
        enabled
    }
}

unsafe fn send(command: &mut Port<u8>, data: &mut Port<u8>, byte: u8) -> bool {
    if !wait_input_clear(command) {
        return false;
    }
    command.write(0xd4);
    if !wait_input_clear(command) {
        return false;
    }
    data.write(byte);
    wait_output_full(command) && data.read() == ACK
}

unsafe fn wait_input_clear(command: &mut Port<u8>) -> bool {
    (0..SPIN_LIMIT).any(|_| command.read() & 2 == 0)
}

unsafe fn wait_output_full(command: &mut Port<u8>) -> bool {
    (0..SPIN_LIMIT).any(|_| command.read() & 1 != 0)
}

pub fn push_byte(byte: u8) {
    if let Some(queue) = BYTES.get() {
        let _ = queue.push(byte);
    }
}

pub struct Decoder {
    packet: [u8; 3],
    length: usize,
}

impl Decoder {
    pub const fn new() -> Self {
        Self {
            packet: [0; 3],
            length: 0,
        }
    }

    pub fn next_event(&mut self) -> Option<MouseEvent> {
        let queue = BYTES.get()?;
        while let Some(byte) = queue.pop() {
            if self.length == 0 && (byte & 0x08 == 0 || byte & 0xc0 != 0) {
                continue;
            }
            self.packet[self.length] = byte;
            self.length += 1;
            if self.length < 3 {
                continue;
            }
            self.length = 0;
            let flags = self.packet[0] as i32;
            let dx = self.packet[1] as i32 - ((flags << 4) & 0x100);
            let dy = self.packet[2] as i32 - ((flags << 3) & 0x100);
            return Some(MouseEvent {
                dx,
                dy: -dy,
                left: flags & 1 != 0,
            });
        }
        None
    }
}
