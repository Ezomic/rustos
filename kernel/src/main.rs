#![no_std]
#![no_main]

mod console;
mod framebuffer;
mod keyboard;

use bootloader_api::{entry_point, BootInfo};
use console::Console;
use core::fmt::Write;
use core::panic::PanicInfo;
use framebuffer::Display;
use keyboard::Keyboard;

const PROMPT: &str = "> ";
const LINE_CAPACITY: usize = 256;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    let Some(framebuffer) = boot_info.framebuffer.as_mut() else {
        halt()
    };
    let info = framebuffer.info();
    let display = Display::new(framebuffer.buffer_mut(), info);
    let mut console = Console::new(display);
    let mut keyboard = Keyboard::new();

    let _ = writeln!(
        console,
        "rustos console, {}x{} cells",
        console.columns(),
        console.rows()
    );
    let _ = writeln!(console, "type help for commands");
    let _ = write!(console, "{PROMPT}");

    let mut line = [0u8; LINE_CAPACITY];
    let mut length = 0;
    loop {
        let Some(ch) = keyboard.poll() else {
            core::hint::spin_loop();
            continue;
        };
        match ch {
            '\n' | '\r' => {
                console.put_char('\n');
                let command = core::str::from_utf8(&line[..length]).unwrap_or("");
                run_command(&mut console, command);
                length = 0;
                let _ = write!(console, "{PROMPT}");
            }
            '\u{8}' | '\u{7f}' => {
                if length > 0 {
                    length -= 1;
                    console.put_char('\u{8}');
                }
            }
            ' '..='~' if length < LINE_CAPACITY => {
                line[length] = ch as u8;
                length += 1;
                console.put_char(ch);
            }
            _ => {}
        }
    }
}

fn run_command(console: &mut Console, command: &str) {
    let (name, argument) = command.split_once(' ').unwrap_or((command, ""));
    match name {
        "" => {}
        "help" => {
            let _ = writeln!(console, "help  clear  echo <text>  info");
        }
        "clear" => console.clear(),
        "echo" => {
            let _ = writeln!(console, "{argument}");
        }
        "info" => {
            let _ = writeln!(
                console,
                "console {}x{} cells",
                console.columns(),
                console.rows()
            );
        }
        _ => {
            let _ = writeln!(console, "unknown command: {name}");
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    halt()
}

fn halt() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
