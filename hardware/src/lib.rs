// SPDX-License-Identifier: GPL-3.0-only

#![no_std]

use stm32h7::stm32h725;

/// Exclusive ownership of the N0120 board hardware.
pub struct Board {
    _core: cortex_m::Peripherals,
    _device: stm32h725::Peripherals,
}

impl Board {
    /// Takes the board hardware once.
    pub fn take() -> Option<Self> {
        Some(Self {
            _core: cortex_m::Peripherals::take()?,
            _device: stm32h725::Peripherals::take()?,
        })
    }
}
