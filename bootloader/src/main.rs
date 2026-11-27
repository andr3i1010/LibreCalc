// SPDX-License-Identifier: GPL-3.0-only

#![no_main]
#![no_std]

mod boot;
mod dfu;
mod epsilon_compat;

use core::{
    fmt::Write,
    ptr::{read_volatile, write_volatile},
};
use cortex_m_rt::entry;
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_9X18},
    pixelcolor::{Rgb565, raw::RawU16},
    prelude::{DrawTarget, Drawable, Point, RgbColor, Size},
    primitives::Rectangle,
    text::Text,
};
use embedded_sprites_macro::include_png_as_rgb565;
use n0120_hardware::{
    Backlight, Console, Display, EXTERNAL_FLASH_END, EXTERNAL_FLASH_START, Key, Led, STOCK_CPU_MHZ,
    battery_is_charging, battery_percentage, battery_voltage, initialize_battery,
    initialize_keyboard, initialize_system, pcb_revision, read_keyboard,
};
use stm32h7::stm32h725::{DBGMCU, FLASH};

// DO NOT REMOVE!!!!!!!! it WILL brick your calculator (from experience)
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
    rcc.ahb4enr()
        .modify(|_, w| w.gpioaen().enabled().gpiocen().enabled());
    let _ = rcc.ahb4enr().read().bits();

    // The keyboard only needs its GPIOs, so 6 can be checked before any other initialization.
    initialize_keyboard();
    if Key::Six.is_pressed(read_keyboard()) {
        dfu::recovery();
        // Recovery is a backup feature in case if initialization of the hardware fails
        // You can still unbrick your unit by holding 6 at boot, if any code past this point fails.
        // If any code before this point fails, good luck soldering an SWD: https://blog.ayabusa.dev/2025/11/07/lets-create-an-os-for-the-numworks-a-different-approach-part-2/
    }

    initialize_system(STOCK_CPU_MHZ);
    let mut backlight = Backlight::new(0);
    let mut display = Display::new();
    let mut led = Led::new();
    backlight.set_level(Backlight::MAX_LEVEL);
    led.set_color(Rgb565::BLACK);

    let mut style = MonoTextStyle::new(&FONT_9X18, Rgb565::WHITE);
    style.background_color = Some(Rgb565::BLACK);
    let slots = [boot::get_slot(0), boot::get_slot(1)];
    let items = ["Boot slot A", "Boot slot B", "DFU", "Power off", "Info"];
    let mut selected = 0usize;
    let mut previous_keys = read_keyboard();
    loop {
        display.clear(Rgb565::BLACK).unwrap();
        display
            .fill_contiguous(
                &Rectangle::new(Point::new(25, 24), Size::new(48, 48)),
                include_png_as_rgb565!("assets/logo.png").map(|p| Rgb565::from(RawU16::new(p))),
            )
            .unwrap();
        display
            .fill_contiguous(
                &Rectangle::new(Point::new(83, 32), Size::new(211, 32)),
                include_png_as_rgb565!("assets/text.png").map(|p| Rgb565::from(RawU16::new(p))),
            )
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
                if Key::Six.is_pressed(keys) {
                    // STM32H725 ROM vector table (the system-memory region starts at 0x1FF00000).
                    const ROM: u32 = 0x1FF0_9800;
                    let stack = unsafe { read_volatile(ROM as *const u32) };
                    let reset = unsafe { read_volatile((ROM + 4) as *const u32) };

                    rcc.cr().modify(|_, w| w.hsion().set_bit().hsidiv().div1());
                    assert!((0..5_000_000).any(|_| {
                        let clocks = rcc.cr().read();
                        clocks.hsirdy().bit_is_set() && clocks.hsidivf().bit_is_set()
                    }));
                    rcc.cfgr().modify(|_, w| w.sw().hsi());
                    assert!((0..5_000_000).any(|_| rcc.cfgr().read().sws().is_hsi()));
                    rcc.d1cfgr()
                        .write(|w| w.d1cpre().div1().hpre().div1().d1ppre().div1());
                    rcc.d2cfgr().write(|w| w.d2ppre1().div1().d2ppre2().div1());
                    rcc.d3cfgr().write(|w| w.d3ppre().div1());
                    rcc.cr().modify(|_, w| {
                        w.pll1on()
                            .clear_bit()
                            .pll2on()
                            .clear_bit()
                            .pll3on()
                            .clear_bit()
                    });
                    assert!((0..5_000_000).any(|_| {
                        let clocks = rcc.cr().read();
                        !clocks.pll1rdy().bit()
                            && !clocks.pll2rdy().bit()
                            && !clocks.pll3rdy().bit()
                    }));

                    unsafe {
                        core::arch::asm!("cpsid i", options(nomem, nostack, preserves_flags));
                        write_volatile(0xE000_E010 as *mut u32, 0); // SysTick CTRL
                        write_volatile(0xE000_E014 as *mut u32, 0); // SysTick LOAD
                        write_volatile(0xE000_E018 as *mut u32, 0); // SysTick VAL
                        write_volatile(0xE000_ED04 as *mut u32, (1 << 25) | (1 << 27));
                        for bank in 0..8 {
                            write_volatile((0xE000_E180usize + bank * 4) as *mut u32, u32::MAX);
                            write_volatile((0xE000_E280usize + bank * 4) as *mut u32, u32::MAX);
                        }
                        write_volatile(0xE000_ED94 as *mut u32, 0); // MPU CTRL
                        write_volatile(0xE000_ED08 as *mut u32, ROM); // VTOR
                        core::arch::asm!("dsb sy", "isb sy", options(nomem, nostack));
                        core::arch::asm!(
                            "msr BASEPRI, r2",
                            "msr FAULTMASK, r2",
                            "msr CONTROL, r2",
                            "isb sy",
                            "msr MSP, {stack}",
                            "cpsie i",
                            "bx {reset}",
                            stack = in(reg) stack,
                            reset = in(reg) reset,
                            in("r2") 0u32,
                            options(noreturn),
                        );
                    }
                }
                dfu::run(&mut display);
            }
            if confirmed && selected == old_selected && selected == 3 {
                n0120_hardware::power_off();
            }
            // Boot slot A, boot slot B or info: DFU and power off never return.
            if confirmed && selected == old_selected {
                display.clear(Rgb565::BLACK).unwrap();
                let mut log = Console::new(
                    &mut display,
                    Rectangle::new(Point::zero(), Size::new(320, 240)),
                    Rgb565::WHITE,
                    Rgb565::BLACK,
                );
                if selected == 4 {
                    let id = unsafe { &*DBGMCU::ptr() }.idc().read();
                    let (dev, rev) = (id.dev_id().bits(), id.rev_id().bits());
                    let [a, b, c] = unsafe { read_volatile(0x1FF1_E800 as *const [u32; 3]) };
                    let flash_kb = unsafe { read_volatile(0x1FF1_E880 as *const u16) };
                    let external_mb = (EXTERNAL_FLASH_END - EXTERNAL_FLASH_START) >> 20;
                    let version = env!("CARGO_PKG_VERSION");
                    writeln!(log, "LibreCalc bootloader {version}\n").unwrap();
                    writeln!(log, "Chip       STM32 {dev:#05x} rev {rev:#06x}").unwrap();
                    writeln!(log, "CPU        {STOCK_CPU_MHZ} MHz").unwrap();
                    writeln!(log, "Flash      {flash_kb} KB + {external_mb} MB external").unwrap();
                    match pcb_revision() {
                        Some(pcb) => writeln!(log, "PCB        {}.{:02}", pcb / 100, pcb % 100),
                        None => writeln!(log, "PCB        blank"),
                    }
                    .unwrap();
                    writeln!(log, "Unique ID  {a:08X}{b:08X}{c:08X}").unwrap();
                    let battery = initialize_battery()
                        .and_then(|()| Ok((battery_voltage()?, battery_percentage()?)));
                    match battery {
                        Ok((volts, percent)) => writeln!(
                            log,
                            "Battery    {} mV, {percent}%{}",
                            (volts * 1000.0) as u32,
                            ["", ", charging"][battery_is_charging() as usize]
                        ),
                        Err(error) => writeln!(log, "Battery    {error}"),
                    }
                    .unwrap();

                    let flash = unsafe { &*FLASH::ptr() };
                    let (options, options2) = (flash.optsr_cur().read(), flash.optsr2_cur().read());
                    let rdp = match options.rdp().bits() {
                        0xAA => 0,
                        0xCC => 2,
                        _ => 1,
                    };
                    let locked = !flash.wpsn_cur().read().wrpsn().bits();
                    let boot = u32::from(flash.boot_cur().read().boot_cm_add0().bits()) << 16;
                    let bor = options.bor_lev().bits();
                    let watchdog = ["hardware", "software"][options.iwdg1_sw().bit() as usize];
                    let itcm = [64, 128, 192, 64][options2.tcm_axi_shared().bits() as usize];
                    let boost = ["off", "on"][options2.cpufreq_boost().bit() as usize];
                    writeln!(log, "\nRead protection  level {rdp}").unwrap();
                    writeln!(log, "Locked sectors   {locked:08b}").unwrap();
                    writeln!(log, "Boot address     {boot:#010x}").unwrap();
                    writeln!(log, "Brown-out level  {bor}").unwrap();
                    writeln!(log, "Watchdog         {watchdog}").unwrap();
                    writeln!(log, "ITCM             {itcm} KB").unwrap();
                    writeln!(log, "CPU boost        {boost}").unwrap();
                } else {
                    writeln!(log, "Boot slot {}", if selected == 0 { "A" } else { "B" }).unwrap();
                    writeln!(log, "Preparing external flash...").unwrap();
                    writeln!(log, "Checking entry at +0x20...").unwrap();
                    let error = boot::boot_slot(boot::SLOTS[selected]).unwrap_err();
                    writeln!(log, "ERROR: {error}").unwrap();
                }
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
