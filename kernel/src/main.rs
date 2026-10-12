#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

mod console;
mod desktop;
mod framebuffer;
mod gdt;
mod gfx;
mod interrupts;
mod keyboard;
mod memory;
mod mouse;
mod shell;

use alloc::format;
use alloc::vec;
use bootloader_api::config::Mapping;
use bootloader_api::{entry_point, BootInfo, BootloaderConfig};
use console::Console;
use core::panic::PanicInfo;
use desktop::Desktop;
use framebuffer::Display;
use keyboard::Keyboard;
use shell::Shell;
use x86_64::instructions::interrupts as cpu;

const TERMINAL_COLUMNS: i32 = 48;
const TERMINAL_ROWS: i32 = 20;

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    let Some(framebuffer) = boot_info.framebuffer.as_mut() else {
        interrupts::halt()
    };
    let info = framebuffer.info();
    let display = Display::new(framebuffer.buffer_mut(), info);

    gdt::init();
    interrupts::init();
    let offset = boot_info
        .physical_memory_offset
        .into_option()
        .expect("physical memory is not mapped");
    memory::init_heap(offset, &boot_info.memory_regions);
    keyboard::init_queue();
    let mouse_ready = mouse::init();

    let console = Console::new(TERMINAL_COLUMNS, TERMINAL_ROWS);
    let terminal_size = (console.canvas().width(), console.canvas().height());
    console::install(console);
    let about = vec![
        format!("rustos 0.3"),
        format!("screen {}x{}", display.width(), display.height()),
        format!("heap {} MiB", memory::HEAP_SIZE >> 20),
        format!("timer {} Hz", interrupts::TICKS_PER_SECOND),
        format!("mouse {}", if mouse_ready { "ready" } else { "not found" }),
    ];
    desktop::install(Desktop::new(display, terminal_size, about));
    cpu::enable();

    println!("type help for commands");
    event_loop()
}

fn event_loop() -> ! {
    let mut keyboard = Keyboard::new();
    let mut mouse = mouse::Decoder::new();
    let mut shell = Shell::new();
    loop {
        cpu::enable_and_hlt();
        while let Some(ch) = keyboard.next_char() {
            if desktop::terminal_focused() {
                shell.handle_char(ch);
            }
        }
        while let Some(event) = mouse.next_event() {
            desktop::mouse_event(&event);
        }
        desktop::tick(interrupts::ticks());
        desktop::redraw();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("panic: {info}");
    desktop::emergency_redraw();
    interrupts::halt()
}
