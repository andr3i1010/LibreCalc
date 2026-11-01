// SPDX-License-Identifier: GPL-3.0-only

#![allow(clippy::result_unit_err)]

use core::ptr::{read_volatile, write_volatile};
use stm32h7::stm32h725::FLASH;

use crate::mcu::{GPIOC, delay_us, set_field};

pub const INTERNAL_FLASH_START: u32 = 0x0800_0000;
pub const INTERNAL_FLASH_END: u32 = 0x0808_0000;
const SECTOR_SIZE: u32 = 128 * 1024;
const METADATA_START: u32 = 0x0807_FA00;
const METADATA_END: u32 = 0x0807_FC00;
const ERRORS: u32 = 0x07EE_0000;

pub fn erase_internal_flash(address: u32) -> Result<(), ()> {
    if !(INTERNAL_FLASH_START..INTERNAL_FLASH_END).contains(&address)
        || !address.is_multiple_of(SECTOR_SIZE)
    {
        return Err(());
    }
    let metadata = unsafe { read_volatile(METADATA_START as *const [u8; 512]) };
    unlock()?;
    let flash = unsafe { &*FLASH::ptr() };
    flash.cr().modify(|_, w| unsafe {
        w.ser()
            .set_bit()
            .snb()
            .bits(((address - INTERNAL_FLASH_START) / SECTOR_SIZE) as u8)
            .start()
            .set_bit()
    });
    let result = wait_ready();
    flash
        .cr()
        .modify(|_, w| w.ser().clear_bit().lock().set_bit());
    result?;
    if address == INTERNAL_FLASH_END - SECTOR_SIZE {
        program_words(METADATA_START, &metadata)?;
    }
    Ok(())
}

pub fn program_internal_flash(address: u32, data: &[u8]) -> Result<(), ()> {
    let end = address.checked_add(data.len() as u32).ok_or(())?;
    if data.is_empty()
        || address < INTERNAL_FLASH_START
        || end > INTERNAL_FLASH_END
        || !address.is_multiple_of(32)
        || address < METADATA_END && end > METADATA_START
    {
        return Err(());
    }
    program_words(address, data)
}

// Stock records this on first boot. Epsilon reads a blank page as a legacy board and hangs on ADC3.
// Only 425 is ever written: the word is permanent and the straps are not trusted enough for 415.
pub fn restore_pcb_revision() -> Result<(), &'static str> {
    if unsafe { read_volatile(METADATA_START as *const u32) } != u32::MAX {
        return Ok(());
    }
    if sample_with_pull_down(3) || !sample_with_pull_down(11) {
        return Err("blank PCB revision, straps not 425");
    }
    program_words(METADATA_START, &(!425u32).to_le_bytes())
        .map_err(|_| "could not write PCB revision")
}

// PC11 is also a keypad column, so its configuration has to survive for the menu.
fn sample_with_pull_down(pin: u32) -> bool {
    let read = |offset| unsafe { read_volatile((GPIOC + offset) as *const u32) };
    let mode = read(0x00) >> (pin * 2) & 3;
    let pull = read(0x0C) >> (pin * 2) & 3;
    set_field(GPIOC, 0x00, pin * 2, 2, 0);
    set_field(GPIOC, 0x0C, pin * 2, 2, 2);
    delay_us(1000);
    let high = read(0x10) & (1 << pin) != 0;
    set_field(GPIOC, 0x00, pin * 2, 2, mode);
    set_field(GPIOC, 0x0C, pin * 2, 2, pull);
    high
}

fn program_words(address: u32, data: &[u8]) -> Result<(), ()> {
    let flash = unsafe { &*FLASH::ptr() };
    for offset in (0..data.len()).step_by(32) {
        if unsafe { read_volatile((address + offset as u32) as *const [u8; 32]) } != [0xFF; 32] {
            return Err(());
        }
    }
    unlock()?;
    let result = cortex_m::interrupt::free(|_| {
        for (index, chunk) in data.chunks(32).enumerate() {
            let address = address + (index * 32) as u32;
            let mut word = [0xFFu8; 32];
            word[..chunk.len()].copy_from_slice(chunk);
            if word == [0xFF; 32] {
                continue;
            }
            flash.cr().modify(|_, w| w.pg().set_bit());
            cortex_m::asm::dsb();
            for (offset, bytes) in word.as_chunks::<4>().0.iter().enumerate() {
                unsafe {
                    write_volatile(
                        (address + (offset * 4) as u32) as *mut u32,
                        u32::from_le_bytes(*bytes),
                    );
                }
            }
            cortex_m::asm::dsb();
            wait_ready()?;
            flash.cr().modify(|_, w| w.pg().clear_bit());
            if unsafe { read_volatile(address as *const [u8; 32]) } != word {
                return Err(());
            }
        }
        Ok(())
    });
    flash
        .cr()
        .modify(|_, w| w.pg().clear_bit().lock().set_bit());
    result
}

fn unlock() -> Result<(), ()> {
    wait_ready()?;
    let flash = unsafe { &*FLASH::ptr() };
    if flash.cr().read().lock().bit_is_set() {
        flash.keyr().write(|w| unsafe { w.bits(0x4567_0123) });
        flash.keyr().write(|w| unsafe { w.bits(0xCDEF_89AB) });
    }
    let unlocked = flash.cr().read().lock().bit_is_clear();
    unlocked.then_some(()).ok_or(())
}

fn wait_ready() -> Result<(), ()> {
    let flash = unsafe { &*FLASH::ptr() };
    for _ in 0..100_000 {
        let status = flash.sr().read().bits();
        if status & 7 == 0 {
            flash
                .ccr()
                .write(|w| unsafe { w.bits(status & (ERRORS | 1 << 16)) });
            return (status & ERRORS == 0).then_some(()).ok_or(());
        }
        delay_us(100);
    }
    Err(())
}
