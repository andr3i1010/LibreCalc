// SPDX-License-Identifier: GPL-3.0-only

#![no_main]
#![no_std]

use core::panic::PanicInfo;
use cortex_m_rt::entry;
use n0120_hardware::Board;

#[entry]
fn main() -> ! {
    let _board = Board::take().unwrap();

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
