// SPDX-License-Identifier: GPL-3.0-only

#![no_main]
#![no_std]

use core::{fmt::Write, panic::PanicInfo};
use cortex_m_rt::entry;
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point, RgbColor, Size},
    primitives::{Rectangle},
};
use n0120_hardware::{Backlight, Board, Console};

#[entry]
fn main() -> ! {
    let mut board = Board::take().unwrap();

    board.display.initialize();
    board.backlight.set_level(Backlight::MAX_LEVEL);

    board.display.clear(Rgb565::BLACK).unwrap();
    let mut console = Console::new(
        Rectangle::new(Point::new(0, 0), Size::new(320, 156)),
        Rgb565::WHITE,
        Rgb565::BLACK,
    );
    let mut writer = console.writer(&mut board.display);
    writeln!(writer, "LibreCalc kernel OK!").unwrap();

    loop {
        cortex_m::asm::wfi();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        cortex_m::asm::wfi();
    }
}
