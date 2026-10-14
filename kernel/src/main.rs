// SPDX-License-Identifier: GPL-3.0-only

#![no_main]
#![no_std]

use core::fmt::Write;
use cortex_m_rt::entry;
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point, RgbColor, Size},
    primitives::Rectangle,
};
use n0120_hardware::{Console, Display, initialize_keyboard, initialize_system};
mod memory;
mod process;
mod svc;

#[used]
#[unsafe(link_section = ".image_header.version")]
static IMAGE_VERSION: [u8; 8] = {
    let version = env!("CARGO_PKG_VERSION").as_bytes();
    assert!(version.len() <= 7, "firmware version must fit in 7 bytes");
    let mut field = [0; 8];
    let mut index = 0;
    while index < version.len() {
        field[index] = version[index];
        index += 1;
    }
    field
};

#[entry]
fn main() -> ! {
    initialize_system();
    initialize_keyboard();

    let mut display = Display::new();
    display.clear(Rgb565::BLACK).unwrap();
    let mut console = Console::new(
        &mut display,
        Rectangle::new(Point::zero(), Size::new(320, 240)),
        Rgb565::WHITE,
        Rgb565::BLACK,
    );
    writeln!(console, "LibreCalc kernel {}", env!("CARGO_PKG_VERSION")).unwrap();
    writeln!(console, "Initializing memory...").unwrap();

    memory::init();
    writeln!(console, "Memory ready.").unwrap();

    let process_id = process::create(memory::PAGE_SIZE).expect("failed to create process");

    assert_eq!(process_id, 1);

    writeln!(console, "Created PID: {process_id}").unwrap();
    writeln!(console, "Current PID: {:?}", process::current_id()).unwrap();
    

    loop {
        cortex_m::asm::wfi();
    }
}
