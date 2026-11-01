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
use n0120_hardware::{Console, Display, FAST_CPU_MHZ, initialize_keyboard, initialize_system};
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
    let mut buffer = [0; 16];
    let version = librecalc_abi::os_version(&mut buffer).unwrap();
    writeln!(console, "LibreCalc Kernel {version}").unwrap();
    writeln!(console, "Initializing memory...").unwrap();

    memory::init();
    writeln!(
        console,
        "Memory ready: {} KiB free.",
        memory::PAGE_COUNT * memory::PAGE_SIZE / 1024
    )
    .unwrap();
    core.SCB.enable_dcache(&mut core.CPUID);
    initialize_keyboard();

    let process_id = process::create(memory::PAGE_SIZE).expect("failed to create process");

    assert_eq!(process_id, 1);

    writeln!(console, "Created PID: {process_id}").unwrap();
    writeln!(console, "Current PID: {:?}", process::current_id()).unwrap();

    loop {
        cortex_m::asm::wfi();
    }
}
