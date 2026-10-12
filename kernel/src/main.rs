#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

mod console;
mod desktop;
mod diagnostics;
mod framebuffer;
mod game;
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
use desktop::{Desktop, Focus};
use framebuffer::Display;
use game::Breakout;
use keyboard::{Key, Keyboard};
use shell::Shell;
use x86_64::instructions::interrupts as cpu;

const TERMINAL_COLUMNS: i32 = 40;
const TERMINAL_ROWS: i32 = 20;
const TALL_SCREEN: i32 = 1800;

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

    let scale = if display.height() >= TALL_SCREEN {
        2
    } else {
        1
    };
    let console = Console::new(TERMINAL_COLUMNS, TERMINAL_ROWS, 2 * scale);
    let terminal_size = (console.canvas().width(), console.canvas().height());
    console::install(console);
    let about = vec![
        format!("rustos 0.4"),
        format!("screen {}x{}", display.width(), display.height()),
        format!("ui scale {scale}x"),
        format!("heap {} MiB", memory::HEAP_SIZE >> 20),
        format!("timer {} Hz", interrupts::TICKS_PER_SECOND),
        format!("mouse {}", if mouse_ready { "ready" } else { "not found" }),
    ];
    game::install(Breakout::new(interrupts::ticks()));
    let game_size = (game::WIDTH, game::HEIGHT);
    desktop::install(Desktop::new(
        display,
        scale,
        terminal_size,
        game_size,
        about,
    ));
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
        while let Some(input) = keyboard.next_input() {
            match (desktop::focus(), input.key) {
                (Focus::Terminal, Key::Char(ch)) => shell.handle_char(ch),
                (Focus::Game, key) => game::key(key, input.pressed),
                _ => {}
            }
        }
        while let Some(event) = mouse.next_event() {
            desktop::mouse_event(&event);
        }
        let now = interrupts::ticks();
        if desktop::focus() == Focus::Game {
            game::update(now);
        } else {
            game::pause(now);
        }
        desktop::tick(now);
        desktop::redraw();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("panic: {info}");
    desktop::emergency_redraw();
    interrupts::halt()
}
