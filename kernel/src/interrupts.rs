use crate::gdt;
use crate::keyboard;
use crate::println;
use core::sync::atomic::{AtomicU64, Ordering};
use pic8259::ChainedPics;
use spin::{LazyLock, Mutex};
use x86_64::instructions::interrupts;
use x86_64::instructions::port::Port;
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

const PIC_OFFSET: u8 = 32;
const TIMER_VECTOR: u8 = PIC_OFFSET;
const KEYBOARD_VECTOR: u8 = PIC_OFFSET + 1;
const PIT_FREQUENCY: u32 = 1_193_182;
pub const TICKS_PER_SECOND: u64 = 100;

static PICS: Mutex<ChainedPics> =
    Mutex::new(unsafe { ChainedPics::new(PIC_OFFSET, PIC_OFFSET + 8) });
static TICKS: AtomicU64 = AtomicU64::new(0);

static IDT: LazyLock<InterruptDescriptorTable> = LazyLock::new(|| {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.page_fault.set_handler_fn(page_fault_handler);
    unsafe {
        idt.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
    }
    idt[TIMER_VECTOR].set_handler_fn(timer_handler);
    idt[KEYBOARD_VECTOR].set_handler_fn(keyboard_handler);
    idt
});

pub fn init() {
    IDT.load();
    unsafe {
        let mut pics = PICS.lock();
        pics.initialize();
        pics.write_masks(0b1111_1100, 0xff);
    }
    set_timer_frequency();
}

pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

fn set_timer_frequency() {
    let divisor = (PIT_FREQUENCY as u64 / TICKS_PER_SECOND) as u16;
    let mut command = Port::<u8>::new(0x43);
    let mut channel0 = Port::<u8>::new(0x40);
    unsafe {
        command.write(0x36);
        channel0.write((divisor & 0xff) as u8);
        channel0.write((divisor >> 8) as u8);
    }
}

fn end_of_interrupt(vector: u8) {
    unsafe { PICS.lock().notify_end_of_interrupt(vector) };
}

extern "x86-interrupt" fn timer_handler(_frame: InterruptStackFrame) {
    TICKS.fetch_add(1, Ordering::Relaxed);
    end_of_interrupt(TIMER_VECTOR);
}

extern "x86-interrupt" fn keyboard_handler(_frame: InterruptStackFrame) {
    let scancode = unsafe { Port::<u8>::new(0x60).read() };
    keyboard::push_scancode(scancode);
    end_of_interrupt(KEYBOARD_VECTOR);
}

extern "x86-interrupt" fn breakpoint_handler(frame: InterruptStackFrame) {
    println!("breakpoint at {:#x}", frame.instruction_pointer.as_u64());
}

extern "x86-interrupt" fn page_fault_handler(
    frame: InterruptStackFrame,
    error: PageFaultErrorCode,
) {
    let address = Cr2::read().map(|a| a.as_u64()).unwrap_or(0);
    println!(
        "page fault at {:#x} accessing {:#x} ({:?})",
        frame.instruction_pointer.as_u64(),
        address,
        error
    );
    halt()
}

extern "x86-interrupt" fn double_fault_handler(frame: InterruptStackFrame, _error: u64) -> ! {
    println!("double fault at {:#x}", frame.instruction_pointer.as_u64());
    halt()
}

pub fn halt() -> ! {
    interrupts::disable();
    loop {
        x86_64::instructions::hlt();
    }
}
