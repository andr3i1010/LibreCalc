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
        Rectangle::new(Point::new(0, 0), Size::new(320, 156)),
        Rgb565::WHITE,
        Rgb565::BLACK,
    );
    n0120_hardware::Led::new().set_color(Rgb565::RED);
    n0120_hardware::Led::new().set_color(Rgb565::GREEN);
    n0120_hardware::Led::new().set_color(Rgb565::BLUE);
    let mut version = [0; 8];
    writeln!(
        console,
        "OS Version: {}",
        librecalc_abi::os_version(&mut version).unwrap()
    )
    .unwrap();
    writeln!(
        console,
        "ABI Version: {}",
        librecalc_abi::abi_version(&mut version).unwrap()
    )
    .unwrap();
    loop {
        cortex_m::asm::wfi();
    }
}
