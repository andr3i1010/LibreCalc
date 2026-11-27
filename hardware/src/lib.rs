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
mod timer;
mod usb;

pub use backlight::Backlight;
pub use battery::{battery_is_charging, battery_percentage, battery_voltage, initialize_battery};
pub use console::Console;
pub use display::{Display, fill_rect, push_rect, wait_for_vblank};
pub use flash::{
    INTERNAL_FLASH_END, INTERNAL_FLASH_START, TRAMPOLINE, Trampoline, erase_internal_flash,
    pcb_revision, program_internal_flash, restore_pcb_revision,
};
pub use keyboard::{KEYBOARD_ROWS, initialize_keyboard, read_keyboard};
pub use led::Led;
pub use librecalc_abi::Key;
pub use mcu::{
    AXI_SRAM, DTCM, EPSILON_CPU_MHZ, FAST_CPU_MHZ, ITCM, SRAM1_2, STOCK_CPU_MHZ, initialize_system,
    power_off, reset, standby, suspend,
};
pub use spi::{
    EXTERNAL_FLASH_END, EXTERNAL_FLASH_START, erase_external_flash, initialize_spi,
    program_external_flash,
};
pub use timer::{initialize_timer, micros, sleep_until};
pub use usb::{initialize_usb, initialize_usb_hardware};
