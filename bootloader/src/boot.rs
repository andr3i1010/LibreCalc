// SPDX-License-Identifier: GPL-3.0-only

use core::ptr::read_volatile;
#[cfg(target_arch = "arm")]
use core::{arch::asm, ptr::write_volatile};
use n0120_hardware::initialize_spi;

pub const SLOTS: [u32; 2] = [0x9000_0000, 0x9040_0000];

const FIRMWARE_START: u32 = 0x400;
const FIRMWARE_LENGTH: u32 = 0x3f_0000;
const LIBRECALC_MAGIC: u32 = u32::from_le_bytes(*b"LCAL");
const EPSILON_MAGIC: u32 = 0xDEC0_0DF0;

pub fn get_slot(slot: usize) -> Option<(&'static str, [u8; 8])> {
    let base = *SLOTS.get(slot)?;
    initialize_spi().ok()?;
    let header = unsafe { read_volatile(base as *const [u8; 32]) };
    let schema = u32::from_le_bytes(header[0..4].try_into().unwrap());
    let image_length = u32::from_le_bytes(header[4..8].try_into().unwrap());
    let magic = u32::from_le_bytes(header[8..12].try_into().unwrap());
    let repeated_magic = u32::from_le_bytes(header[28..32].try_into().unwrap());
    if magic != repeated_magic || !(0x28..=FIRMWARE_LENGTH).contains(&image_length) {
        return None;
    }

    let name = match (schema, magic) {
        (1, LIBRECALC_MAGIC) => "LibreCalc",
        (2, EPSILON_MAGIC) => "Epsilon",
        _ => return None,
    };
    let mut version: [u8; 8] = header[12..20].try_into().unwrap();
    let length = version.iter().position(|byte| *byte == 0).unwrap_or(8);
    if length == 0 || !version[..length].iter().all(u8::is_ascii_graphic) {
        return None;
    }
    version[length..].fill(0);
    Some((name, version))
}

#[cfg(target_arch = "arm")]
pub fn boot_slot(base: u32) -> Result<(), &'static str> {
    initialize_spi()?;
    let stack = unsafe { read_volatile((base + 0x20) as *const u32) };
    let reset = unsafe { read_volatile((base + 0x24) as *const u32) };
    let stack_valid = stack & 7 == 0
        && ((0x2000_0000..=0x2002_0000).contains(&stack)
            || (0x2400_0000..=0x2404_0000).contains(&stack));
    if !stack_valid {
        return Err("invalid initial stack pointer");
    }
    if reset & 1 == 0 || !(base + FIRMWARE_START..base + FIRMWARE_LENGTH).contains(&(reset & !1)) {
        return Err("invalid reset address for this slot");
    }

    unsafe {
        asm!("cpsid i", options(nomem, nostack, preserves_flags));
        write_volatile(0xE000_E010 as *mut u32, 0); // SysTick CTRL
        write_volatile(0xE000_E014 as *mut u32, 0); // SysTick LOAD
        write_volatile(0xE000_E018 as *mut u32, 0); // SysTick VAL
        write_volatile(0xE000_ED04 as *mut u32, (1 << 25) | (1 << 27));
        for bank in 0..8 {
            write_volatile((0xE000_E180usize + bank * 4) as *mut u32, u32::MAX);
            write_volatile((0xE000_E280usize + bank * 4) as *mut u32, u32::MAX);
        }
        write_volatile(0xE000_ED94 as *mut u32, 0); // MPU CTRL
        asm!("dsb sy", "isb sy", options(nomem, nostack));
        asm!(
            "msr BASEPRI, r2",
            "msr FAULTMASK, r2",
            "msr CONTROL, r2",
            "isb sy",
            "msr MSP, {stack}",
            "cpsie i",
            "bx {reset}",
            stack = in(reg) stack,
            reset = in(reg) reset,
            in("r2") 0u32,
            options(noreturn),
        );
    }
}
