// SPDX-License-Identifier: GPL-3.0-only

#![no_main]
#![no_std]

mod boot;
mod epsilon_compat;

use core::fmt::Write;
use cortex_m_rt::entry;
use embedded_graphics::{
    image::{Image, ImageRawLE},
    mono_font::{MonoTextStyle, ascii::FONT_9X18},
    pixelcolor::Rgb565,
    prelude::{
        DrawTarget, 
        Drawable, 
        Point, 
        RgbColor, 
        Size
    },
    primitives::Rectangle,
    text::Text,
};
use n0120_hardware::{
    Backlight, 
    Console, 
    Display, 
    Key, 
    initialize_keyboard, 
    initialize_system, 
    read_keyboard
};

#[entry]
fn main() -> ! {
    initialize_system();
    initialize_keyboard();
    let mut backlight = Backlight::new(0);
    let mut display = Display::new();
    backlight.set_level(Backlight::MAX_LEVEL);

    let mut style = MonoTextStyle::new(&FONT_9X18, Rgb565::WHITE);
    style.background_color = Some(Rgb565::BLACK);
    let slots = [boot::get_slot(0), boot::get_slot(1)];
    let items = ["Boot slot A", "Boot slot B", "DFU", "Power off"];
    let mut selected = 0usize;
    let mut previous_keys = read_keyboard();
    loop {
        display.clear(Rgb565::BLACK).unwrap();
        Image::new(
            &ImageRawLE::<Rgb565>::new(&include_bytes!("../assets/smiley.bmp")[70..], 64),
            Point::new(248, 8),
        )
        .draw(&mut display)
        .unwrap();
        for (row, label) in items.into_iter().enumerate() {
            let mut position = Text::new(label, Point::new(34, 114 + row as i32 * 24), style)
                .draw(&mut display)
                .unwrap();
            if let Some(slot) = slots.get(row) {
                let parts: &[&str] = match slot {
                    Some((name, version)) => &[
                        " (",
                        name,
                        " ",
                        core::str::from_utf8(version)
                            .unwrap()
                            .trim_end_matches('\0'),
                        ")",
                    ],
                    None => &[" (Unknown)"],
                };
                for part in parts {
                    position = Text::new(part, position, style).draw(&mut display).unwrap();
                }
            }
        }
        Text::new(">", Point::new(16, 114 + selected as i32 * 24), style)
            .draw(&mut display)
            .unwrap();

        loop {
            let keys = read_keyboard();
            let old_selected = selected;

            if Key::Up.is_pressed(keys) && !Key::Up.is_pressed(previous_keys) {
                selected = selected.saturating_sub(1);
            } else if Key::Down.is_pressed(keys) && !Key::Down.is_pressed(previous_keys) {
                selected = (selected + 1).min(items.len() - 1);
            }
            let confirmed = Key::OK.is_pressed(keys) && !Key::OK.is_pressed(previous_keys);
            previous_keys = keys;

            if selected != old_selected {
                for (row, marker) in [(old_selected, " "), (selected, ">")] {
                    Text::new(marker, Point::new(16, 114 + row as i32 * 24), style)
                        .draw(&mut display)
                        .unwrap();
                }
            }

            if confirmed && selected == old_selected && selected < boot::SLOTS.len() {
                display.clear(Rgb565::BLACK).unwrap();
                let mut log = Console::new(
                    &mut display,
                    Rectangle::new(Point::zero(), Size::new(320, 240)),
                    Rgb565::WHITE,
                    Rgb565::BLACK,
                );
                writeln!(log, "Boot slot {}", if selected == 0 { "A" } else { "B" }).unwrap();
                writeln!(log, "Preparing external flash...").unwrap();
                writeln!(log, "Checking entry at +0x20...").unwrap();
                let error = boot::boot_slot(boot::SLOTS[selected]).unwrap_err();
                writeln!(log, "ERROR: {error}").unwrap();
                writeln!(log, "Press BACK to go back").unwrap();

                let mut back_was_pressed = Key::Back.is_pressed(keys);
                loop {
                    let current = read_keyboard();
                    let back_pressed = Key::Back.is_pressed(current);
                    if back_pressed && !back_was_pressed {
                        previous_keys = current;
                        break;
                    }
                    back_was_pressed = back_pressed;
                }
                break;
            }
        }
    }
}
