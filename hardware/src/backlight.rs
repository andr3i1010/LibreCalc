// SPDX-License-Identifier: GPL-3.0-only

use core::ptr::write_volatile;

use crate::mcu::delay_us;

const PIN: u32 = 3;
const PULSE_US: u32 = 20;

pub struct Backlight(u8);

impl Backlight {
    pub const MAX_LEVEL: u8 = 15;

    pub fn new(level: u8) -> Self {
        cortex_m::interrupt::free(|_| {
            write_pin(1 << PIN);
            delay_us(50);
            let mut backlight = Self(Self::MAX_LEVEL);
            backlight.set_level(level);
            backlight
        })
    }

    pub fn set_level(&mut self, level: u8) {
        let target = level.min(Self::MAX_LEVEL);
        cortex_m::interrupt::free(|_| {
            for _ in 0..((self.0 + Self::MAX_LEVEL + 1 - target) & Self::MAX_LEVEL) {
                write_pin(1 << (PIN + 16));
                delay_us(PULSE_US);
                write_pin(1 << PIN);
                delay_us(PULSE_US);
            }
        });
        self.0 = target;
    }
}

fn write_pin(bits: u32) {
    unsafe {
        write_volatile(0x5802_0C18 as *mut u32, bits);
    }
}
