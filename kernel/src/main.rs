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
use n0120_hardware::{
    Console, Display, FAST_CPU_MHZ, initialize_battery, initialize_keyboard, initialize_system,
    initialize_timer,
};
mod keyboard;
mod memory;
mod process;
mod settings;
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
    let mut core = cortex_m::Peripherals::take().unwrap();
    core.SCB.enable_icache();
    initialize_system(FAST_CPU_MHZ);

    let mut display = Display::new();
    display.clear(Rgb565::BLACK).unwrap();
    let mut console = Console::new(
        &mut display,
        Rectangle::new(Point::zero(), Size::new(320, 240)),
        Rgb565::WHITE,
        Rgb565::BLACK,
    );
    writeln!(console, "Initializing memory...").unwrap();

    memory::init();
    let (mut os, mut abi) = ([0; 16], [0; 16]);
    let version = librecalc_abi::os_version(&mut os).unwrap();
    let abi = librecalc_abi::abi_version(&mut abi).unwrap();
    writeln!(console, "LibreCalc Kernel {version} (ABI {abi})").unwrap();
    writeln!(
        console,
        "Memory ready: {} KiB free.",
        memory::PAGE_COUNT * memory::PAGE_SIZE / 1024
    )
    .unwrap();
    core.SCB.enable_dcache(&mut core.CPUID);
    initialize_keyboard();
    initialize_timer();
    let _ = initialize_battery();
    process::init();

    writeln!(console, "Starting home...").unwrap();
    let _ = librecalc_abi::exit(0);
    unreachable!("exit returned to the boot thread")
}
