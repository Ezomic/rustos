use crate::println;
use alloc::vec::Vec;

const LINE_CAPACITY: usize = 256;
const PROMPT: &str = "> ";

pub struct Shell {
    line: Vec<u8>,
}

impl Shell {
    pub fn new() -> Self {
        crate::print!("{PROMPT}");
        Self {
            line: Vec::with_capacity(LINE_CAPACITY),
        }
    }

    pub fn handle_char(&mut self, ch: char) {
        match ch {
            '\n' | '\r' => {
                println!();
                run_command(core::str::from_utf8(&self.line).unwrap_or(""));
                self.line.clear();
                crate::print!("{PROMPT}");
            }
            '\u{8}' | '\u{7f}' => {
                if self.line.pop().is_some() {
                    crate::print!("\u{8}");
                }
            }
            ' '..='~' if self.line.len() < LINE_CAPACITY => {
                self.line.push(ch as u8);
                crate::print!("{ch}");
            }
            _ => {}
        }
    }
}

fn run_command(command: &str) {
    let (name, argument) = command.split_once(' ').unwrap_or((command, ""));
    match name {
        "" => {}
        "help" => println!("help  clear  echo <text>  info  uptime  mem  int3  fault"),
        "clear" => {
            crate::console::with_console(|c| c.clear());
        }
        "echo" => println!("{argument}"),
        "info" => {
            let (width, height) = crate::desktop::screen_size();
            println!("screen {width}x{height}");
        }
        "uptime" => {
            let ticks = crate::interrupts::ticks();
            let per_second = crate::interrupts::TICKS_PER_SECOND;
            println!("{}.{:02} s", ticks / per_second, ticks % per_second);
        }
        "mem" => println!(
            "heap used {} of {} KiB",
            crate::memory::heap_used() >> 10,
            crate::memory::HEAP_SIZE >> 10
        ),
        "int3" => x86_64::instructions::interrupts::int3(),
        "fault" => unsafe { (0xdead_0000 as *mut u8).write_volatile(0) },
        _ => println!("unknown command: {name}"),
    }
}
