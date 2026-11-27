// SPDX-License-Identifier: GPL-3.0-only

use core::ptr::read_volatile;
#[cfg(target_arch = "arm")]
use core::{arch::asm, ptr::write_volatile};
use n0120_hardware::{AXI_SRAM, DTCM, EXTERNAL_FLASH_END, EXTERNAL_FLASH_START, initialize_spi};
#[cfg(target_arch = "arm")]
use n0120_hardware::{
    EPSILON_CPU_MHZ, FAST_CPU_MHZ, initialize_battery, initialize_system, initialize_usb_hardware,
    restore_pcb_revision,
};

pub const SLOTS: [u32; 2] = [
    EXTERNAL_FLASH_START,
    EXTERNAL_FLASH_START + (EXTERNAL_FLASH_END - EXTERNAL_FLASH_START) / 2,
];

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
    let epsilon = unsafe { read_volatile((base + 8) as *const u32) } == EPSILON_MAGIC;
    if epsilon {
        // Only Epsilon's v1 bootloader header is known to leave LibreCalc installed.
        let header = unsafe { read_volatile((base + 0x1_0244) as *const [u32; 3]) };
        if header != [0xDEC0_B007, 1, 0xDEC0_B007] {
            return Err("Epsilon may replace LibreCalc bootloader");
        }
    }
    let stack_valid = stack & 7 == 0
        && [DTCM, AXI_SRAM]
            .iter()
            .any(|ram| (ram.start..=ram.end).contains(&stack));
    if !stack_valid {
        return Err("invalid initial stack pointer");
    }
    if reset & 1 == 0 || !(base + FIRMWARE_START..base + FIRMWARE_LENGTH).contains(&(reset & !1)) {
        return Err("invalid reset address for this slot");
    }
    if epsilon {
        initialize_system(EPSILON_CPU_MHZ);
        restore_pcb_revision()?;
        initialize_battery()?;
        initialize_usb_hardware();
        let otg = unsafe { &*stm32h7::stm32h725::OTG1_HS_GLOBAL::ptr() };
        let device = unsafe { &*stm32h7::stm32h725::OTG1_HS_DEVICE::ptr() };
        let power = unsafe { &*stm32h7::stm32h725::OTG1_HS_PWRCLK::ptr() };
        // Leave the embedded full-speed PHY and device core ready for Epsilon.
        power.pcgcr().write(|w| w.stppclk().clear_bit());
        if !(0..5_000_000).any(|_| otg.grstctl().read().ahbidl().bit_is_set()) {
            return Err("USB bus idle timeout");
        }
        otg.gusbcfg()
            .write(|w| w.physel().set_bit().fdmod().set_bit());
        cortex_m::asm::delay(1000);
        if !(0..5_000_000).any(|_| otg.grstctl().read().ahbidl().bit_is_set()) {
            return Err("USB PHY bus idle timeout");
        }
        otg.grstctl().write(|w| w.csrst().set_bit());
        if !(0..5_000_000).any(|_| otg.grstctl().read().csrst().bit_is_clear()) {
            return Err("USB controller reset timeout");
        }
        otg.gccfg()
            .write(|w| w.pwrdwn().set_bit().vbden().set_bit());
        otg.gusbcfg().modify(|_, w| unsafe { w.trdt().bits(6) });
        device.dcfg().write(|w| unsafe { w.dspd().bits(3) });
        otg.grxfsiz().write(|w| unsafe { w.rxfd().bits(128) });
        otg.gahbcfg().write(|w| w.gintmsk().set_bit());
        otg.gintmsk().write(|w| {
            w.enumdnem()
                .set_bit()
                .rxflvlm()
                .set_bit()
                .iepint()
                .set_bit()
        });
        device.daintmsk().write(|w| unsafe { w.iepm().bits(1) });
        device.diepmsk().write(|w| w.xfrcm().set_bit());
    } else {
        initialize_system(FAST_CPU_MHZ);
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
