// SPDX-License-Identifier: GPL-3.0-only

#![no_std]

mod backlight;
mod console;
mod display;

pub use backlight::{Backlight, MAX_LEVEL};
pub use console::{Console, ConsoleWriter};
pub use display::{Display, HEIGHT, WIDTH};
use stm32h7::stm32h725;
/// Exclusive ownership of the N0120 board hardware.
pub struct Board {
    pub backlight: Backlight,
    pub display: Display,
    _core: cortex_m::Peripherals,
    _device: stm32h725::Peripherals,
}

impl Board {
    /// Takes the board hardware once.
    pub fn take() -> Option<Self> {
        Some(Self {
            backlight: Backlight::new(),
            display: Display::new(),
            _core: cortex_m::Peripherals::take()?,
            _device: stm32h725::Peripherals::take()?,
        })
    }
}
