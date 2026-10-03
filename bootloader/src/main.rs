// SPDX-License-Identifier: GPL-3.0-only

#![no_main]
#![no_std]

use core::fmt::Write;
use cortex_m_rt::entry;
use embedded_graphics::{
    image::{Image, ImageRawLE},
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Drawable, Point, RgbColor, Size},
    primitives::Rectangle,
};
use n0120_hardware::{
    Backlight, Console, Display, Key, initialize_keyboard, initialize_system, scan_keyboard,
};

#[entry]
fn main() -> ! {
    initialize_system();
    initialize_keyboard();
    let mut backlight = Backlight::new(0);
    let mut display = Display::new();
    backlight.set_level(Backlight::MAX_LEVEL);

    display.clear(Rgb565::BLACK).unwrap();
    Image::new(
        &ImageRawLE::<Rgb565>::new(&include_bytes!("../assets/smiley.bmp")[70..], 64),
        Point::new(248, 8),
    )
    .draw(&mut display)
    .unwrap();

    let mut console = Console::new(
        &mut display,
        Rectangle::new(Point::new(0, 84), Size::new(320, 156)),
        Rgb565::WHITE,
        Rgb565::BLACK,
    );
    let mut previous = 0;
    loop {
        let current = scan_keyboard();
        let mut on_off_pressed = false;
        for &key in Key::ALL {
            let is_pressed = key.is_pressed(current);
            if key.is_pressed(previous) != is_pressed {
                writeln!(
                    console,
                    "{} {}",
                    key.name(),
                    if is_pressed { "pressed" } else { "released" }
                )
                .unwrap();
                on_off_pressed |= key == Key::OnOff && is_pressed;
            }
        }
        previous = current;
        assert!(!on_off_pressed, "OnOff pressed");
    }
}
