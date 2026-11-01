// SPDX-License-Identifier: GPL-3.0-only

#![no_std]

mod backlight;
mod battery;
mod console;
#[cfg(target_os = "none")]
mod crash;
mod display;
mod flash;
mod keyboard;
mod led;
mod mcu;
mod spi;
mod usb;

pub use backlight::Backlight;
pub use battery::{battery_is_charging, battery_percentage, battery_voltage, initialize_battery};
pub use console::Console;
pub use display::Display;
pub use flash::{
    INTERNAL_FLASH_END, INTERNAL_FLASH_START, erase_internal_flash, program_internal_flash,
    restore_pcb_revision,
};
pub use keyboard::{Key, initialize_keyboard, read_keyboard};
pub use led::Led;
pub use mcu::{
    EPSILON_CPU_MHZ, FAST_CPU_MHZ, STOCK_CPU_MHZ, enable_backup_sram, initialize_system, power_off,
    reset, suspend,
};
pub use spi::{
    EXTERNAL_FLASH_END, EXTERNAL_FLASH_START, erase_external_flash, initialize_spi,
    program_external_flash,
};
pub use usb::{initialize_usb, initialize_usb_hardware};
