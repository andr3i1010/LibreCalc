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
use n0120_hardware::{Console, Display};

#[entry]
fn main() -> ! {
    let mut display = Display::new();

    display.clear(Rgb565::BLACK).unwrap();
    let mut console = Console::new(
        &mut display,
        Rectangle::new(Point::new(0, 0), Size::new(320, 156)),
        Rgb565::WHITE,
        Rgb565::BLACK,
    );
    writeln!(console, "LibreCalc kernel OK!").unwrap();

    loop {
        cortex_m::asm::wfi();
    }
}
