use crate::{desktop, interrupts, keyboard, memory, mouse, println};
use x86_64::instructions::port::Port;

pub fn print() {
    let status = unsafe { Port::<u8>::new(0x64).read() };
    println!("framebuffer {}", desktop::display_summary());
    println!("ui scale {}x", desktop::ui_scale());
    println!("ticks {}", interrupts::ticks());
    println!("scancodes seen {}", keyboard::scancodes_seen());
    println!("mouse bytes seen {}", mouse::bytes_seen());
    println!("i8042 status {status:#04x}");
    println!(
        "heap used {} of {} KiB",
        memory::heap_used() >> 10,
        memory::HEAP_SIZE >> 10
    );
}
