#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

mod console;
mod framebuffer;
mod gdt;
mod interrupts;
mod keyboard;
mod memory;

use alloc::vec::Vec;
use bootloader_api::config::Mapping;
use bootloader_api::{entry_point, BootInfo, BootloaderConfig};
use console::Console;
use core::panic::PanicInfo;
use framebuffer::Display;
use keyboard::Keyboard;
use x86_64::instructions::interrupts as cpu;

const PROMPT: &str = "> ";
const LINE_CAPACITY: usize = 256;

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
    console::install(Console::new(Display::new(framebuffer.buffer_mut(), info)));
    let (columns, rows) = console::with_console(|c| (c.columns(), c.rows())).unwrap_or((0, 0));
    println!("rustos console, {columns}x{rows} cells");

    gdt::init();
    interrupts::init();
    let offset = boot_info
        .physical_memory_offset
        .into_option()
        .expect("physical memory is not mapped");
    memory::init_heap(offset, &boot_info.memory_regions);
    keyboard::init_queue();
    cpu::enable();
    println!(
        "heap {} MiB, timer {} Hz",
        memory::HEAP_SIZE >> 20,
        interrupts::TICKS_PER_SECOND
    );
    println!("type help for commands");

    shell()
}

fn shell() -> ! {
    let mut keyboard = Keyboard::new();
    let mut line = Vec::with_capacity(LINE_CAPACITY);
    print!("{PROMPT}");
    loop {
        let Some(ch) = next_char(&mut keyboard) else {
            continue;
        };
        match ch {
            '\n' | '\r' => {
                println!();
                run_command(core::str::from_utf8(&line).unwrap_or(""));
                line.clear();
                print!("{PROMPT}");
            }
            '\u{8}' | '\u{7f}' => {
                if line.pop().is_some() {
                    print!("\u{8}");
                }
            }
            ' '..='~' if line.len() < LINE_CAPACITY => {
                line.push(ch as u8);
                print!("{ch}");
            }
            _ => {}
        }
    }
}

fn next_char(keyboard: &mut Keyboard) -> Option<char> {
    cpu::disable();
    if !keyboard::has_input() {
        cpu::enable_and_hlt();
    } else {
        cpu::enable();
    }
    keyboard.next_char()
}

fn run_command(command: &str) {
    let (name, argument) = command.split_once(' ').unwrap_or((command, ""));
    match name {
        "" => {}
        "help" => println!("help  clear  echo <text>  info  uptime  mem  int3  fault"),
        "clear" => {
            console::with_console(|c| c.clear());
        }
        "echo" => println!("{argument}"),
        "info" => {
            let (columns, rows) =
                console::with_console(|c| (c.columns(), c.rows())).unwrap_or((0, 0));
            println!("console {columns}x{rows} cells");
        }
        "uptime" => {
            let ticks = interrupts::ticks();
            let per_second = interrupts::TICKS_PER_SECOND;
            println!("{}.{:02} s", ticks / per_second, ticks % per_second);
        }
        "mem" => println!(
            "heap used {} of {} KiB",
            memory::heap_used() >> 10,
            memory::HEAP_SIZE >> 10
        ),
        "int3" => x86_64::instructions::interrupts::int3(),
        "fault" => unsafe { (0xdead_0000 as *mut u8).write_volatile(0) },
        _ => println!("unknown command: {name}"),
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("panic: {info}");
    interrupts::halt()
}
