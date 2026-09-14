// SPDX-License-Identifier: GPL-3.0-only

#![no_main]
#![no_std]

use core::panic::PanicInfo;
use cortex_m_rt::entry;
use n0120_hardware::Board;

#[entry]
fn main() -> ! {
    let mut board = Board::take().unwrap();

    board.display.initialize();
    board.display.fill_rgb565(0x0000); // Black
    board.display.fill_rect(20, 20, 80, 200, 0xF800); // Red
    board.display.fill_rect(120, 20, 80, 200, 0x07E0); // Green
    board.display.fill_rect(220, 20, 80, 200, 0x001F); // Blue

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
