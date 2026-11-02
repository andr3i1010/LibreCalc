use core::ptr::{read_volatile, write_volatile};
use cortex_m::peripheral::{DCB, DWT};
use stm32h7::stm32h725;

use crate::mcu::{
    EPSILON_CPU_MHZ, GPIOB, GPIOD, GPIOJ, ahb_frequency, delay_us, set_alternate_function,
    set_field,
};

pub const EXTERNAL_FLASH_START: u32 = 0x9000_0000;
pub const EXTERNAL_FLASH_END: u32 = 0x9080_0000;

const COMMAND_TIMEOUT: usize = 1_000_000;

pub fn initialize_spi() -> Result<(), &'static str> {
    let rcc = unsafe { &*stm32h725::RCC::ptr() };
    let ospi = unsafe { &*stm32h725::OCTOSPI1::ptr() };

    let control = ospi.cr().read();
    if control.en().is_enabled() && control.fmode().is_memory_mapped() {
        return Ok(());
    }

    rcc.ahb4enr().modify(|_, w| w.gpiojen().enabled());
    rcc.ahb3enr().modify(|_, w| w.octospi1en().enabled());
    let _ = rcc.ahb3enr().read().bits();

    // Stock drives PB11 high before using the flash (bootloader 0x8005bd8): it powers the chip.
    // Without it the flash leaks enough supply through its I/Os to read, but erases and programs
    // abort after a few bytes. Latch the level before switching to output to avoid a low glitch.
    if (unsafe { read_volatile(GPIOB as *const u32) } >> 22) & 3 != 1 {
        unsafe { write_volatile((GPIOB + 0x18) as *mut u32, 1 << 11) };
        set_field(GPIOB, 0x00, 22, 2, 1);
        delay_us(10_000); // AT25SF641B power-up time before write commands
    }

    for (port, pin, alternate_function, speed) in [
        (GPIOB, 2, 9, 2),
        (GPIOB, 10, 9, 2),
        (GPIOB, 12, 12, 2),
        (GPIOB, 13, 4, 2),
        (GPIOD, 12, 9, 2),
        (GPIOD, 13, 9, 2),
        (GPIOJ, 15, 15, 3),
    ] {
        set_field(port, 0x00, pin * 2, 2, 2);
        set_field(port, 0x08, pin * 2, 2, speed);
        set_alternate_function(port, pin, alternate_function);
    }

    wait_controller_idle()?;

    ospi.cr().write(|w| unsafe { w.bits(0) });
    ospi.dcr1()
        .write(|w| unsafe { w.devsize().bits(22).csht().bits(5) });
    ospi.dcr2().write(|w| unsafe { w.prescaler().bits(1) });
    ospi.cr().write(|w| w.en().enabled());
    write_command(0xFF, None, &[], 0x0000_0001)?;
    write_command(0xAB, None, &[], 0x0000_0001)?;
    delay_us(3);

    let status2 = read_byte_command(0x35)?;
    if status2 & 2 == 0 {
        write_command(0x06, None, &[], 0x0000_0001)?;
        write_command(0x31, None, &[status2 | 2], 0x0100_0001)?;
        wait_flash_ready::<true>()?;
    }

    enter_memory_mapped()
}

// Epsilon reclaims bootloader RAM; false keeps its flash calls independent of that RAM.
pub fn erase_external_flash<const BOOTLOADER: bool>(address: u32) -> Result<(), &'static str> {
    if BOOTLOADER {
        initialize_spi()?;
    }
    let offset = address
        .checked_sub(EXTERNAL_FLASH_START)
        .filter(|offset| *offset < EXTERNAL_FLASH_END - EXTERNAL_FLASH_START)
        .ok_or("invalid external flash erase address")?;
    let (instruction, length) = if offset < 0x8000 && offset & 0xFFF == 0 {
        (0x20, 0x1000)
    } else if offset == 0x8000 {
        (0x52, 0x8000)
    } else if offset >= 0x1_0000 && offset & 0xFFFF == 0 {
        (0xD8, 0x1_0000)
    } else {
        return Err("invalid external flash erase address");
    };

    flash_operation(address, length, || {
        leave_memory_mapped()?;
        clear_block_protection::<BOOTLOADER>()?;
        write_enable()?;
        write_command(instruction, Some(offset), &[], 0x0000_2101)?;
        check_accepted("erase command ignored")?;
        wait_flash_ready::<BOOTLOADER>()
    })
}

pub fn program_external_flash<const BOOTLOADER: bool>(
    address: u32,
    data: &[u8],
) -> Result<(), &'static str> {
    let end = address
        .checked_add(data.len() as u32)
        .ok_or("invalid external flash program range")?;
    if data.is_empty() || address < EXTERNAL_FLASH_START || end > EXTERNAL_FLASH_END {
        return Err("invalid external flash program range");
    }
    if BOOTLOADER {
        initialize_spi()?;
    }
    let mut offset = 0;
    while offset < data.len() {
        let address = address + offset as u32;
        let length = (data.len() - offset).min(256 - (address as usize & 255));
        flash_operation(address, length, || {
            // The source may be external flash, so stage it before unmapping reads.
            let mut page = [core::mem::MaybeUninit::<u8>::uninit(); 256];
            for (target, source) in page.iter_mut().zip(&data[offset..offset + length]) {
                target.write(unsafe { read_volatile(source) });
            }
            let page = unsafe { core::slice::from_raw_parts(page.as_ptr().cast(), length) };
            leave_memory_mapped()?;
            clear_block_protection::<BOOTLOADER>()?;
            write_enable()?;
            // Stock firmware tries both quad-program variants for BOM compatibility.
            let offset = address - EXTERNAL_FLASH_START;
            write_command(0x33, Some(offset), page, 0x0300_2301)?;
            write_command(0x32, Some(offset), page, 0x0300_2101)?;
            check_accepted("program command ignored")?;
            wait_flash_ready::<BOOTLOADER>()
        })?;
        offset += length;
    }
    Ok(())
}

fn wait_controller_idle() -> Result<(), &'static str> {
    let ospi = unsafe { &*stm32h725::OCTOSPI1::ptr() };
    for _ in 0..COMMAND_TIMEOUT {
        if ospi.sr().read().busy().bit_is_clear() {
            return Ok(());
        }
        cortex_m::asm::nop();
    }
    Err("OCTOSPI controller stayed busy")
}

fn wait_transfer() -> Result<(), &'static str> {
    let ospi = unsafe { &*stm32h725::OCTOSPI1::ptr() };
    for _ in 0..COMMAND_TIMEOUT {
        let status = ospi.sr().read();
        if status.tef().bit_is_set() {
            return Err("OCTOSPI transfer error");
        }
        if status.tcf().bit_is_set() {
            return Ok(());
        }
        cortex_m::asm::nop();
    }
    Err("OCTOSPI transfer timed out")
}

fn write_command(
    instruction: u8,
    address: Option<u32>,
    data: &[u8],
    configuration: u32,
) -> Result<(), &'static str> {
    let ospi = unsafe { &*stm32h725::OCTOSPI1::ptr() };
    ospi.fcr().write(|w| unsafe { w.bits(0x1F) });
    ospi.cr().write(|w| w.en().enabled());
    ospi.dlr()
        .write(|w| unsafe { w.bits(data.len().saturating_sub(1) as u32) });
    ospi.tcr().write(|w| unsafe { w.bits(0) });
    ospi.ccr().write(|w| unsafe { w.bits(configuration) });
    ospi.ir().write(|w| unsafe { w.bits(instruction as u32) });
    if let Some(address) = address {
        ospi.ar().write(|w| unsafe { w.bits(address) });
    }
    for byte in data {
        unsafe { write_volatile(ospi.dr().as_ptr().cast::<u8>(), *byte) };
    }
    wait_transfer()?;
    ospi.fcr().write(|w| unsafe { w.bits(0x1F) });
    // BUSY outlasts TCF while nCS is held; registers written meanwhile are silently ignored.
    wait_controller_idle()
}

fn read_byte_command(instruction: u8) -> Result<u8, &'static str> {
    let ospi = unsafe { &*stm32h725::OCTOSPI1::ptr() };
    ospi.fcr().write(|w| unsafe { w.bits(0x1F) });
    ospi.cr()
        .write(|w| w.en().enabled().fmode().indirect_read());
    ospi.dlr().write(|w| unsafe { w.bits(0) });
    ospi.tcr().write(|w| unsafe { w.bits(0) });
    ospi.ccr()
        .write(|w| w.imode().single_line().dmode().single_line());
    ospi.ir().write(|w| unsafe { w.bits(instruction as u32) });
    wait_transfer()?;
    let value = unsafe { read_volatile(ospi.dr().as_ptr().cast::<u8>()) };
    ospi.fcr().write(|w| unsafe { w.bits(0x1F) });
    wait_controller_idle()?;
    Ok(value)
}

fn wait_flash_ready<const BOOTLOADER: bool>() -> Result<(), &'static str> {
    // The singleton API writes bootloader RAM that Epsilon may have reused.
    unsafe {
        (*DCB::PTR).demcr.modify(|r| r | 1 << 24);
        DWT::unlock();
        (*DWT::PTR).ctrl.modify(|r| r | 1);
    }
    let timeout = if BOOTLOADER {
        ahb_frequency() * 2 * 5
    } else {
        // Epsilon reuses bootloader RAM; its maximum CPU rate gives at least five seconds.
        EPSILON_CPU_MHZ * 5_000_000
    };
    let start = DWT::cycle_count();
    loop {
        if read_byte_command(0x05)? & 1 == 0 {
            return Ok(());
        }
        if DWT::cycle_count().wrapping_sub(start) >= timeout {
            return Err("external flash stayed busy");
        }
    }
}

// Older Epsilon/Upsilon builds lock slots (BP/TB/SEC, CMP) and the flash then silently ignores
// erases and programs; stock N0120 likewise unlocks before erasing. Done here, not in
// initialize_spi, because the boot menu maps the flash before DFU initializes it.
fn clear_block_protection<const BOOTLOADER: bool>() -> Result<(), &'static str> {
    let status1 = read_byte_command(0x05)?;
    let status2 = read_byte_command(0x35)?;
    if status1 & 0x7C == 0 && status2 & 0x40 == 0 {
        return Ok(());
    }
    write_enable()?;
    // Keep QE; also drop SRP1 so the protection cannot be relocked until power-down.
    write_command(0x01, None, &[0, status2 & !0x41], 0x0100_0001)?;
    wait_flash_ready::<BOOTLOADER>()
}

fn write_enable() -> Result<(), &'static str> {
    write_command(0x06, None, &[], 0x0000_0001)?;
    if read_byte_command(0x05)? & 2 == 0 {
        return Err("write enable not latched");
    }
    Ok(())
}

// A dropped erase/program leaves WEL set without BUSY; the flash reports nothing else.
fn check_accepted(error: &'static str) -> Result<(), &'static str> {
    let status = read_byte_command(0x05)?;
    if status & 3 == 2 {
        return Err(error);
    }
    Ok(())
}

fn leave_memory_mapped() -> Result<(), &'static str> {
    let ospi = unsafe { &*stm32h725::OCTOSPI1::ptr() };
    if ospi.cr().read().fmode().is_memory_mapped() {
        ospi.cr().modify(|_, w| w.abort().requested());
        wait_controller_idle()?;
    }
    ospi.cr().write(|w| w.en().enabled());
    // Aborting the controller does not release the flash from instruction-less reads.
    write_command(0xFF, None, &[], 0x0000_0001)
}

fn enter_memory_mapped() -> Result<(), &'static str> {
    wait_controller_idle()?;
    let ospi = unsafe { &*stm32h725::OCTOSPI1::ptr() };
    ospi.fcr().write(|w| unsafe { w.bits(0x1F) });
    ospi.cr()
        .write(|w| w.en().enabled().fmode().memory_mapped());
    ospi.dlr().write(|w| unsafe { w.bits(0x007F_FFFF) });
    ospi.tcr().write(|w| unsafe { w.dcyc().bits(4) });
    ospi.ccr().write(|w| {
        w.imode()
            .single_line()
            .admode()
            .four_lines()
            .adsize()
            .bits24()
            .abmode()
            .four_lines()
            .dmode()
            .four_lines()
    });
    // Every mapped transaction sends an instruction, so keep continuous reads disabled.
    ospi.abr().write(|w| unsafe { w.bits(0xFF) });
    ospi.ir().write(|w| unsafe { w.bits(0xEB) });
    cortex_m::asm::dsb();
    cortex_m::asm::isb();
    Ok(())
}

pub(crate) fn flash_operation(
    address: u32,
    length: usize,
    operation: impl FnOnce() -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    let ospi = unsafe { &*stm32h725::OCTOSPI1::ptr() };
    let control = ospi.cr().read();
    if !control.en().is_enabled() || !control.fmode().is_memory_mapped() {
        return Err("external flash is not mapped");
    }
    cortex_m::interrupt::free(|_| {
        let control = ospi.cr().read().bits();
        let data_length = ospi.dlr().read().bits();
        let timing = ospi.tcr().read().bits();
        let configuration = ospi.ccr().read().bits();
        let alternate = ospi.abr().read().bits();
        let instruction = ospi.ir().read().bits();
        let result = operation();

        ospi.cr().modify(|_, w| w.abort().requested());
        wait_ready(|| ospi.sr().read().busy().bit_is_clear());
        ospi.cr().write(|w| w.en().enabled());
        unsafe {
            ospi.fcr().write(|w| w.bits(0x1F));
            ospi.dlr().write(|w| w.bits(data_length));
            ospi.tcr().write(|w| w.bits(timing));
            ospi.ccr().write(|w| w.bits(configuration));
            ospi.abr().write(|w| w.bits(alternate));
            ospi.cr().write(|w| w.bits(control));
            ospi.ir().write(|w| w.bits(instruction));

            cortex_m::asm::dsb();
            let cache = read_volatile(0xE000_ED14 as *const u32);
            if cache & (1 << 16) != 0 {
                for line in (address as usize & !31..address as usize + length).step_by(32) {
                    write_volatile(0xE000_EF5C as *mut u32, line as u32);
                }
            }
            if cache & (1 << 17) != 0 {
                write_volatile(0xE000_EF50 as *mut u32, 0);
            }
        }
        cortex_m::asm::dsb();
        cortex_m::asm::isb();
        result
    })
}

pub(crate) fn wait_ready(mut ready: impl FnMut() -> bool) {
    if !(0..1_000_000).any(|_| ready()) {
        // Epsilon may have reclaimed bootloader RAM; reset without its panic handler.
        unsafe {
            write_volatile(0xE000_ED0C as *mut u32, 0x05FA_0004);
            core::arch::asm!("dsb sy", "2: wfi", "b 2b", options(noreturn, nostack));
        }
    }
}
