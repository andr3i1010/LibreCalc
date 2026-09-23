// SPDX-License-Identifier: GPL-3.0-only

#![no_main]
#![no_std]

use core::{fmt::Write, panic::PanicInfo};
use cortex_m_rt::entry;
use embedded_graphics::{
    image::Image,
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Drawable, Point, RgbColor, Size},
    primitives::{Rectangle},
};
use n0120_hardware::{Backlight, Board, Console};
use tinybmp::Bmp;

const SMILEY: &[u8] = include_bytes!("../assets/smiley.bmp");
const TICK_DELAY_CYCLES: u32 = 10_000_000;

#[entry]
fn main() -> ! {
    let mut board = Board::take().unwrap();

    board.backlight.initialize(0);
    board.display.initialize();

    board.display.clear(Rgb565::BLACK).unwrap();
    let _bmp = match Bmp::<Rgb565>::from_slice(SMILEY) {
        Ok(bmp) => {
            Image::new(&bmp, Point::new(248, 8))
                .draw(&mut board.display)
                .unwrap();
            "BMP: RGB565"
        }
        Err(_) => "BMP: malformed",
    };

    let mut console = Console::new(
        Rectangle::new(Point::new(0, 0), Size::new(320, 240)),
        Rgb565::WHITE, // Text color
        Rgb565::BLACK, // BG color
    );
    board.backlight.set_level(Backlight::MAX_LEVEL);

    let mut tick = 0u32;
    loop {
        cortex_m::asm::delay(TICK_DELAY_CYCLES);
        tick = tick.wrapping_add(1);
        let mut writer = console.writer(&mut board.display);
        writeln!(writer, "tick {tick}").unwrap();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        cortex_m::asm::wfi();
    }
}