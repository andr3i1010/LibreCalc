// SPDX-License-Identifier: GPL-3.0-only

use core::ptr::write_volatile;
use stm32h7::stm32h725;

use crate::display::{delay_us, set_field};

const GPIOD: usize = 0x5802_0C00;
const GPIO_BSRR: usize = GPIOD + 0x18;
const PIN: u32 = 3;
const SYNC_HIGH_US: u32 = 50;
const PULSE_US: u32 = 20;
pub const MAX_LEVEL: u8 = 15;

/// Independent 16-level control for the N0120 LCD backlight.
pub struct Backlight {
    level: u8,
}

impl Backlight {
    pub const MAX_LEVEL: u8 = MAX_LEVEL;

    pub(crate) const fn new() -> Self {
        Self { level: MAX_LEVEL }
    }

    pub fn initialize(&mut self, level: u8) {
        let rcc = unsafe { &*stm32h725::RCC::ptr() };
        rcc.ahb4enr().modify(|_, w| w.gpioden().enabled());
        let _ = rcc.ahb4enr().read().bits();

        set_field(GPIOD, 0x00, PIN * 2, 2, 1); // Output mode.
        set_field(GPIOD, 0x04, PIN, 1, 0); // Push-pull.
        set_field(GPIOD, 0x08, PIN * 2, 2, 1); // Medium speed.

        self.level = MAX_LEVEL;
        self.apply(clamp_level(level), true);
    }

    pub fn set_level(&mut self, level: u8) {
        self.apply(clamp_level(level), false);
    }

    pub const fn level(&self) -> u8 {
        self.level
    }

    fn apply(&mut self, target: u8, synchronize: bool) {
        let pulses = pulse_count(self.level, target);
        cortex_m::interrupt::free(|_| {
            if synchronize {
                write_pin(1 << PIN);
                // ponytail: CPU_HZ is the timing calibration knob for physical validation.
                delay_us(SYNC_HIGH_US);
            }
            for _ in 0..pulses {
                write_pin(1 << (PIN + 16));
                delay_us(PULSE_US);
                write_pin(1 << PIN);
                delay_us(PULSE_US);
            }
        });
        self.level = target;
    }
}

fn write_pin(bits: u32) {
    unsafe {
        write_volatile(GPIO_BSRR as *mut u32, bits);
    }
}

const fn clamp_level(level: u8) -> u8 {
    if level > MAX_LEVEL { MAX_LEVEL } else { level }
}

const fn pulse_count(current: u8, target: u8) -> u8 {
    (current + MAX_LEVEL + 1 - target) & MAX_LEVEL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamping_and_pulse_counts_match_the_protocol() {
        assert_eq!(clamp_level(42), MAX_LEVEL);
        assert_eq!(pulse_count(15, 15), 0);
        assert_eq!(pulse_count(15, 0), 15);
        assert_eq!(pulse_count(0, 15), 1);
        assert_eq!(pulse_count(11, 4), 7);
    }
}
