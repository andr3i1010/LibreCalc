// SPDX-License-Identifier: GPL-3.0-only

#![no_main]
#![no_std]

mod boot;
mod dfu;
mod epsilon_compat;

use core::fmt::Write;
use cortex_m_rt::entry;
use embedded_graphics::{
    image::{Image, ImageRawLE},
    mono_font::{MonoTextStyle, ascii::FONT_9X18},
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Drawable, Point, RgbColor, Size},
    primitives::Rectangle,
    text::Text,
};
use n0120_hardware::{
    Backlight, 
    Console, 
    Display, 
    Key, 
    Led, 
    STOCK_CPU_MHZ, 
    initialize_keyboard, 
    initialize_system, 
    read_keyboard,
};

#[unsafe(no_mangle)]
#[unsafe(naked)]
#[unsafe(link_section = ".Reset")]
unsafe extern "C" fn __pre_init() {
    core::arch::naked_asm!(
        "ldr r0, =0x5802480C", // PWR_CR3: leave Run* without changing voltage scaling
        "ldr r1, [r0]",
        "bic r1, r1, #0x3F",
        "orr r1, r1, #4",
        "str r1, [r0]",
        "sub r0, r0, #8", // PWR_CSR1: wait for ACTVOSRDY
        "1:",
        "ldr r1, [r0]",
        "tst r1, #0x2000",
        "beq 1b",
        "bx lr",
    );
}

#[entry]
fn main() -> ! {
    unsafe extern "C" {
        static __vector_table: u32;
        static mut __ram_vectors: u32;
    }
    unsafe {
        let vectors = &raw mut __ram_vectors;
        core::ptr::copy_nonoverlapping(&raw const __vector_table, vectors, 0x400 / 4);
        core::ptr::write_volatile(0xE000_ED08 as *mut u32, vectors as u32); // VTOR
        core::arch::asm!("dsb sy", "isb sy", options(nostack));
    }

    let rcc = unsafe { &*stm32h7::stm32h725::RCC::ptr() };
    let rows = unsafe { &*stm32h7::stm32h725::GPIOA::ptr() };
    let columns = unsafe { &*stm32h7::stm32h725::GPIOC::ptr() };
    rcc.ahb4enr()
        .modify(|_, w| w.gpioaen().enabled().gpiocen().enabled());
    let _ = rcc.ahb4enr().read().bits();

    // Only PA8/PC11 are needed to sample 6 before normal hardware initialization.
    columns
        .moder()
        .modify(|r, w| unsafe { w.bits(r.bits() & !(3 << 22)) });
    columns
        .pupdr()
        .modify(|r, w| unsafe { w.bits((r.bits() & !(3 << 22)) | (1 << 22)) });
    rows.bsrr().write(|w| unsafe { w.bits(1 << 24) });
    rows.otyper()
        .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 8)) });
    rows.pupdr()
        .modify(|r, w| unsafe { w.bits(r.bits() & !(3 << 16)) });
    rows.moder()
        .modify(|r, w| unsafe { w.bits((r.bits() & !(3 << 16)) | (1 << 16)) });
    cortex_m::asm::delay(4096);
    rows.bsrr().write(|w| unsafe { w.bits(1 << 8) });
    if columns.idr().read().bits() & (1 << 11) == 0 { //Key 6
        dfu::recovery();
        // Recovery is a backup feature in case if initialization of the hardware fails
        // You can still unbrick your unit by holding 6 at boot
    }

    initialize_system(STOCK_CPU_MHZ);
    initialize_keyboard();
    let mut backlight = Backlight::new(0);
    let mut display = Display::new();
    let mut led = Led::new();
    backlight.set_level(Backlight::MAX_LEVEL);
    led.set_color(Rgb565::BLACK);

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
            if confirmed && selected == old_selected && selected == 2 {
                dfu::run(&mut display);
            }
            if confirmed && selected == old_selected && selected == 3 {
                n0120_hardware::power_off();
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
