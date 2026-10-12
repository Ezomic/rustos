#![no_std]
#![no_main]

use bootloader_api::{entry_point, BootInfo};
use core::panic::PanicInfo;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
        let info = framebuffer.info();
        let bytes_per_pixel = info.bytes_per_pixel;
        let stride = info.stride;
        let buffer = framebuffer.buffer_mut();
        for y in 0..info.height {
            for x in 0..info.width {
                let offset = (y * stride + x) * bytes_per_pixel;
                let r = (x * 255 / info.width) as u8;
                let g = (y * 255 / info.height) as u8;
                buffer[offset] = 160;
                buffer[offset + 1] = g;
                buffer[offset + 2] = r;
            }
        }
    }
    halt()
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
