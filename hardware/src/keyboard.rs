// SPDX-License-Identifier: GPL-3.0-only

use core::ptr::write_volatile;
use stm32h7::stm32h725;

use crate::mcu::{GPIOA, GPIOC, delay_us, set_field};

/// Row pins on GPIOA, in key order.
pub const KEYBOARD_ROWS: [u8; 9] = [1, 0, 2, 3, 6, 7, 8, 10, 15];
/// Column pins on GPIOC, in key order (boards from 4.20 on; older ones used PC2/PC3 for 9/11).
pub const KEYBOARD_COLUMNS: [u8; 6] = [1, 9, 11, 4, 5, 6];

pub fn initialize_keyboard() {
    for pin in KEYBOARD_ROWS.map(u32::from) {
        // Open-drain outputs, released (high) until a scan pulls one row low.
        unsafe { write_volatile((GPIOA + 0x18) as *mut u32, 1 << pin) };
        set_field(GPIOA, 0x04, pin, 1, 1);
        set_field(GPIOA, 0x0C, pin * 2, 2, 0);
        set_field(GPIOA, 0x00, pin * 2, 2, 1);
    }
    for pin in KEYBOARD_COLUMNS.map(u32::from) {
        // Inputs with pull-ups: a pressed key reads low.
        set_field(GPIOC, 0x0C, pin * 2, 2, 1);
        set_field(GPIOC, 0x00, pin * 2, 2, 0);
    }
}

pub fn read_keyboard() -> u64 {
    let rows = unsafe { &*stm32h725::GPIOA::ptr() };
    let columns = unsafe { &*stm32h725::GPIOC::ptr() };
    let mut bits = 0;

    for (row, pin) in KEYBOARD_ROWS.into_iter().enumerate() {
        rows.bsrr().write(|w| unsafe { w.bits(1 << (pin + 16)) });
        delay_us(100);
        let input = columns.idr().read().bits();
        for (column, pin) in KEYBOARD_COLUMNS.into_iter().enumerate() {
            if input & (1 << pin) == 0 {
                bits |= 1 << (row * KEYBOARD_COLUMNS.len() + column);
            }
        }
        rows.bsrr().write(|w| unsafe { w.bits(1 << pin) });
    }

    bits
}
