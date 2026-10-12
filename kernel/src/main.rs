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
mod memory;
mod process;
mod svc;

unsafe extern "C" {
    static _user_ram_start: u8;
    static _user_ram_end: u8;
}

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
        Rectangle::new(Point::zero(), Size::new(320, 240)),
        Rgb565::WHITE,
        Rgb565::BLACK,
    );
    writeln!(console, "LibreCalc kernel {}", env!("CARGO_PKG_VERSION")).unwrap();
    writeln!(console, "Initializing memory...").unwrap();

    let mut memory = unsafe {
        memory::Memory::new(
            (&raw const _user_ram_start) as usize,
            (&raw const _user_ram_end) as usize,
        )
    };
    writeln!(console, "Memory ready.").unwrap();

    let process_id = 1;
    let stack_size = memory::PAGE_SIZE;

    let stack_start = memory
        .alloc(process_id, stack_size)
        .expect("failed to allocate test stack");

    let stack_end = stack_start + stack_size;
    writeln!(console, "Stack allocated: {stack_size} bytes").unwrap();
    writeln!(console, "Start: 0x{stack_start:08X}").unwrap();
    writeln!(console, "End:   0x{stack_end:08X}").unwrap();

    let stack_ptr = stack_end - core::mem::size_of::<cortex_m_rt::ExceptionFrame>();
    assert!(stack_ptr >= stack_start && stack_ptr % 8 == 0);
    writeln!(console, "CPU frame: 0x{stack_ptr:08X}").unwrap();

    let frame = unsafe { 
        &mut *(stack_ptr as *mut cortex_m_rt::ExceptionFrame)
    };

    unsafe {
        frame.set_xpsr(1 << 24);
    }

    writeln!(console, "CPU flags: 0x{:08X}", frame.xpsr()).unwrap();

    loop {
        cortex_m::asm::wfi();
    }
}
