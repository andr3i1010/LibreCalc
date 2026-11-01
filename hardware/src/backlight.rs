// SPDX-License-Identifier: GPL-3.0-only

use crate::mcu::{BACKLIGHT_PIN, delay_us};

const PULSE_US: u32 = 20;

pub struct Backlight(u8);

impl Backlight {
    pub const MAX_LEVEL: u8 = 15;

    pub fn new(level: u8) -> Self {
        let port = unsafe { &*stm32h7::stm32h725::GPIOD::ptr() };
        cortex_m::interrupt::free(|_| {
            port.bsrr().write(|w| unsafe { w.bits(1 << BACKLIGHT_PIN) });
            delay_us(50);
            let mut backlight = Self(Self::MAX_LEVEL);
            backlight.set_level(level);
            backlight
        })
    }

    pub fn set_level(&mut self, level: u8) {
        let port = unsafe { &*stm32h7::stm32h725::GPIOD::ptr() };
        let target = level.min(Self::MAX_LEVEL);
        cortex_m::interrupt::free(|_| {
            for _ in 0..((self.0 + Self::MAX_LEVEL + 1 - target) & Self::MAX_LEVEL) {
                port.bsrr()
                    .write(|w| unsafe { w.bits(1 << (BACKLIGHT_PIN + 16)) });
                delay_us(PULSE_US);
                port.bsrr().write(|w| unsafe { w.bits(1 << BACKLIGHT_PIN) });
                delay_us(PULSE_US);
            }
        });
        self.0 = target;
    }
}
