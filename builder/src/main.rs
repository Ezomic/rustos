use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let kernel = PathBuf::from(args.next().expect("usage: builder <kernel-elf> <output-img>"));
    let output = PathBuf::from(args.next().expect("usage: builder <kernel-elf> <output-img>"));
    bootloader::UefiBoot::new(&kernel)
        .create_disk_image(&output)
        .expect("failed to create UEFI disk image");
}
