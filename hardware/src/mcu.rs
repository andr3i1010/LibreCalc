// SPDX-License-Identifier: GPL-3.0-only

use core::{
    ptr::{read_volatile, write_volatile},
    sync::atomic::{AtomicU32, Ordering},
};
use stm32h7::stm32h725::{self, rcc::d1ccipr::FMCSEL};

pub const GPIOA: usize = 0x5802_0000;
pub const GPIOB: usize = 0x5802_0400;
pub const GPIOC: usize = 0x5802_0800;
pub const GPIOD: usize = 0x5802_0C00;
pub const GPIOE: usize = 0x5802_1000;
pub const GPIOJ: usize = 0x5802_2400;
pub const BACKLIGHT_PIN: u32 = 3;
pub const STOCK_CPU_MHZ: u32 = 270;
pub const FAST_CPU_MHZ: u32 = 520;
pub const EPSILON_CPU_MHZ: u32 = 540;

static CPU_MHZ: AtomicU32 = AtomicU32::new(64);

pub(crate) fn ahb_frequency() -> u32 {
    CPU_MHZ.load(Ordering::Relaxed) * 500_000
}

// here comes the ai slop, hope you (don't) enjoy
// yall really think a 14 year old can reverse engineer an entire proprietary system on their own?

pub fn initialize_system(mhz: u32) {
    assert!(mhz == STOCK_CPU_MHZ || mhz == FAST_CPU_MHZ || mhz == EPSILON_CPU_MHZ);
    let fast = mhz != STOCK_CPU_MHZ;
    // CSI is 4 MHz; the wide VCO wants a 2 MHz reference, stock uses 1 MHz
    let divm: u8 = if fast { 2 } else { 4 };
    let divn = (mhz * u32::from(divm) / 4 - 1) as u16;

    let flash = unsafe { &*stm32h725::FLASH::ptr() };
    let pwr = unsafe { &*stm32h725::PWR::ptr() };
    let rcc = unsafe { &*stm32h725::RCC::ptr() };
    let ospi = unsafe { &*stm32h725::OCTOSPI1::ptr() };
    // Keep the rescue firmware's flash timing while mapped reads are active.
    let control = ospi.cr().read();
    let preserve_ospi_clock = !fast
        && control.en().is_enabled()
        && control.fmode().is_memory_mapped()
        && rcc.d1ccipr().read().octospisel().is_pll2_r();

    // Preserve a running PLL when the kernel enters from the bootloader.
    let clocks = rcc.cr().read();
    let pll = rcc.pllcfgr().read();
    let dividers = rcc.pll1divr().read();
    let cpu = rcc.d1cfgr().read();
    let buses = rcc.d2cfgr().read();
    let voltage = pwr.d3cr().read();
    let timing = flash.acr().read();
    let clock_configured = rcc.cfgr().read().sw().is_pll1()
        && rcc.cfgr().read().sws().is_pll1()
        && clocks.csion().bit_is_set()
        && clocks.pll1on().bit_is_set()
        && clocks.pll1rdy().bit_is_set()
        && rcc.pllckselr().read().pllsrc().is_csi()
        && rcc.pllckselr().read().divm1().bits() == divm
        && pll.divp1en().is_enabled()
        && pll.divq1en().bit() == fast
        && pll.divr1en().is_disabled()
        && pll.pll1fracen().bit_is_clear()
        && pll.pll1vcosel().is_wide_vco()
        && pll.pll1rge().is_range1() != fast
        && rcc.pll1fracr().read().fracn1().bits() == 0
        && dividers.divn1().bits() == divn
        && dividers.divp1().is_div1()
        && (!fast || dividers.divq1().bits() == 3)
        && cpu.d1cpre().is_div1()
        && cpu.hpre().is_div2()
        && cpu.d1ppre().is_div2()
        && buses.d2ppre1().is_div2()
        && buses.d2ppre2().is_div2()
        && rcc.d3cfgr().read().d3ppre().is_div2()
        && pwr.cr3().read().bits() & 0x3F == 4 // Internal SMPS supply
        && pwr.csr1().read().actvosrdy().bit_is_set()
        && voltage.vos().bits() == 0
        && voltage.vosrdy().bit_is_set()
        && timing.latency().bits() == 3
        && timing.wrhighfreq().bits() == 3
        && rcc.d1ccipr().read().octospisel().is_pll1_q() == fast
        && rcc.d1ccipr().read().fmcsel().is_pll1_q() == fast;

    if !clock_configured {
        // Keep LCD diagnostics running while PLL1 (the stock FMC source) is stopped.
        rcc.d1ccipr().modify(|_, w| w.fmcsel().rcc_hclk3());
        rcc.cr().modify(|_, w| w.hsion().set_bit());
        wait_ready("HSI", || rcc.cr().read().hsirdy().bit_is_set());
        rcc.cfgr().modify(|_, w| w.sw().hsi());
        wait_ready("HSI clock switch", || rcc.cfgr().read().sws().is_hsi());

        rcc.cr().modify(|_, w| w.pll1on().clear_bit());
        wait_ready("PLL1 stop", || rcc.cr().read().pll1rdy().bit_is_clear());

        pwr.cr3().modify(|_, w| unsafe {
            w.bypass()
                .clear_bit()
                .ldoen()
                .clear_bit()
                .sden()
                .set_bit()
                .sdexthp()
                .clear_bit()
                .sdlevel()
                .bits(0)
        });
        wait_ready("power supply", || {
            pwr.csr1().read().actvosrdy().bit_is_set()
        });

        pwr.d3cr().modify(|_, w| unsafe { w.vos().bits(0) });
        wait_ready("voltage scaling", || {
            pwr.d3cr().read().vosrdy().bit_is_set()
        });

        flash
            .acr()
            .modify(|_, w| unsafe { w.latency().bits(3).wrhighfreq().bits(3) });
        wait_ready("flash timing", || {
            let timing = flash.acr().read();
            timing.latency().bits() == 3 && timing.wrhighfreq().bits() == 3
        });

        rcc.cr().modify(|_, w| w.csion().set_bit());
        wait_ready("CSI", || rcc.cr().read().csirdy().bit_is_set());
        rcc.pllckselr()
            .modify(|_, w| unsafe { w.pllsrc().csi().divm1().bits(divm) });
        // The faster profiles use PLL1 Q for the display and external flash.
        rcc.pllcfgr().modify(|_, w| unsafe {
            w.pll1fracen()
                .clear_bit()
                .pll1vcosel()
                .wide_vco()
                .pll1rge()
                .bits(u8::from(fast))
                .divp1en()
                .enabled()
                .divq1en()
                .bit(fast)
                .divr1en()
                .disabled()
        });
        rcc.pll1divr()
            .modify(|_, w| unsafe { w.divn1().bits(divn).divp1().div1().divq1().bits(3) });

        rcc.d1cfgr()
            .modify(|_, w| w.d1cpre().div1().hpre().div2().d1ppre().div2());
        rcc.d2cfgr()
            .modify(|_, w| w.d2ppre1().div2().d2ppre2().div2());
        rcc.d3cfgr().modify(|_, w| w.d3ppre().div2());

        rcc.cr().modify(|_, w| w.pll1on().set_bit());
        wait_ready("PLL1 lock", || rcc.cr().read().pll1rdy().bit_is_set());
        // switch kernel clocks before SYSCLK so OCTOSPI never sees 260 MHz
        let source = if fast {
            FMCSEL::Pll1Q
        } else {
            FMCSEL::RccHclk3
        };
        rcc.d1ccipr().modify(|_, w| {
            w.fmcsel().variant(source);
            if !preserve_ospi_clock {
                w.octospisel().variant(source);
            }
            w
        });
        rcc.cfgr().modify(|_, w| w.sw().pll1());
        wait_ready("PLL1 clock switch", || rcc.cfgr().read().sws().is_pll1());
    }
    CPU_MHZ.store(mhz, Ordering::Relaxed);

    let fmc = unsafe { &*stm32h725::FMC::ptr() };
    let syscfg = unsafe { &*stm32h725::SYSCFG::ptr() };

    rcc.apb4enr().modify(|_, w| w.syscfgen().enabled());
    let _ = rcc.apb4enr().read().bits();
    syscfg.cccsr().modify(|_, w| w.en().set_bit());
    wait_ready("I/O compensation", || {
        syscfg.cccsr().read().ready().bit_is_set()
    });

    let mpu = unsafe { &*cortex_m::peripheral::MPU::PTR };
    cortex_m::asm::dsb();
    unsafe {
        mpu.ctrl.write(0);
        mpu.rnr.write(0);
        mpu.rbar.write(0x6000_0000);
        mpu.rasr
            .write((1 << 28) | (1 << 24) | (2 << 19) | (27 << 1) | 1);
        mpu.ctrl.write(5); // Enable MPU with the privileged default map.
    }
    cortex_m::asm::dsb();
    cortex_m::asm::isb();

    rcc.ahb4enr().modify(|_, w| {
        w.gpioaen()
            .enabled()
            .gpioben()
            .enabled()
            .gpiocen()
            .enabled()
            .gpioden()
            .enabled()
            .gpioeen()
            .enabled()
    });
    rcc.ahb3enr().modify(|_, w| w.fmcen().enabled());

    rcc.ahb2enr()
        .modify(|_, w| w.sram1en().set_bit().sram2en().set_bit());
    let _ = rcc.ahb4enr().read().bits();

    for (port, pins, alternate_function) in [
        (GPIOA, &[4, 5][..], 12),
        (GPIOB, &[14, 15], 12),
        (GPIOC, &[0], 1),
        (GPIOC, &[7], 9),
        (GPIOC, &[12], 1),
        (GPIOD, &[0, 1], 12),
        (GPIOD, &[2], 1),
        (GPIOD, &[4, 5, 8, 9, 10, 11, 14, 15], 12),
        (GPIOE, &[7, 8], 12),
    ] {
        for &pin in pins {
            set_field(port, 0x08, pin * 2, 2, 1);
            set_field(port, 0x00, pin * 2, 2, 2);
            set_alternate_function(port, pin, alternate_function);
        }
    }

    set_field(GPIOE, 0x00, 4 * 2, 2, 1);
    set_field(GPIOE, 0x00, 5 * 2, 2, 1);
    set_field(GPIOC, 0x00, 13 * 2, 2, 1);
    unsafe {
        write_volatile((GPIOE + 0x18) as *mut u32, (1 << 4) | (1 << 5));
        write_volatile((GPIOC + 0x18) as *mut u32, 1 << 13);
    }

    set_field(GPIOB, 0x08, 2, 2, 1);
    set_field(GPIOB, 0x00, 2, 2, 0);
    set_field(GPIOB, 0x0C, 2, 2, 0);

    fmc.bcr1().write(|w| unsafe { w.bits(0x0000_5010) });
    fmc.btr1().write(|w| unsafe { w.bits(0x0000_430F) });
    fmc.bwtr1().write(|w| unsafe { w.bits(0x0000_0606) });
    fmc.bcr1()
        .modify(|_, w| w.mbken().enabled().fmcen().enabled());
    cortex_m::asm::dsb();

    set_field(GPIOD, 0x00, BACKLIGHT_PIN * 2, 2, 1); // Output mode.
    set_field(GPIOD, 0x04, BACKLIGHT_PIN, 1, 0); // Push-pull.
    set_field(GPIOD, 0x08, BACKLIGHT_PIN * 2, 2, 1); // Medium speed.
}

// Separate from initialize_system, which also runs before Epsilon: it must keep the reset state.
pub fn enable_backup_sram() {
    let rcc = unsafe { &*stm32h725::RCC::ptr() };
    let pwr = unsafe { &*stm32h725::PWR::ptr() };
    rcc.ahb4enr().modify(|_, w| w.bkpramen().enabled());
    let _ = rcc.ahb4enr().read().bits();
    pwr.cr1().modify(|_, w| w.dbp().set_bit());
    let _ = pwr.cr1().read().bits();
}

fn wait_ready(name: &str, mut ready: impl FnMut() -> bool) {
    if !(0..5_000_000).any(|_| ready()) {
        panic!("timeout waiting for {name}");
    }
}

pub fn reset() -> ! {
    cortex_m::peripheral::SCB::sys_reset()
}

pub extern "C" fn suspend() {
    #[cfg(target_arch = "arm")]
    let _ = crate::spi::flash_operation(0, 0, || {
        let rcc = unsafe { &*stm32h725::RCC::ptr() };
        let ospi = unsafe { &*stm32h725::OCTOSPI1::ptr() };
        let pll2 = rcc.cr().read().pll2on().bit_is_set();

        // Stop mapped reads before removing their PLL clock; the wrapper restores them.
        ospi.cr().modify(|_, w| w.abort().requested());
        crate::spi::wait_ready(|| ospi.sr().read().busy().bit_is_clear());
        ospi.cr().write(|w| w.en().enabled());

        rcc.cr().modify(|_, w| w.hsion().set_bit().hsidiv().div8());
        crate::spi::wait_ready(|| rcc.cr().read().hsirdy().bit_is_set());
        rcc.cfgr().modify(|_, w| w.sw().hsi());
        crate::spi::wait_ready(|| rcc.cfgr().read().sws().is_hsi());
        rcc.cr().modify(|_, w| {
            w.pll1on()
                .clear_bit()
                .pll2on()
                .clear_bit()
                .hsi48on()
                .clear_bit()
        });
        crate::spi::wait_ready(|| {
            let clocks = rcc.cr().read();
            clocks.pll1rdy().bit_is_clear()
                && clocks.pll2rdy().bit_is_clear()
                && clocks.hsi48rdy().bit_is_clear()
        });

        // shortcut: flash remains powered; add chip power-down if standby current needs reducing.
        unsafe { core::arch::asm!("dsb sy", "sev", "wfe", "nop", "wfe", options(nostack)) };

        rcc.cr()
            .modify(|_, w| w.csion().set_bit().hsi48on().set_bit());
        crate::spi::wait_ready(|| {
            let clocks = rcc.cr().read();
            clocks.csion().bit_is_set() && clocks.hsi48rdy().bit_is_set()
        });
        rcc.d1cfgr().modify(|_, w| w.d1cpre().div1().hpre().div2());
        rcc.cr()
            .modify(|_, w| w.pll1on().set_bit().pll2on().bit(pll2));
        crate::spi::wait_ready(|| {
            let clocks = rcc.cr().read();
            clocks.pll1rdy().bit_is_set() && (!pll2 || clocks.pll2rdy().bit_is_set())
        });
        rcc.cfgr().modify(|_, w| w.sw().pll1());
        crate::spi::wait_ready(|| rcc.cfgr().read().sws().is_pll1());
        Ok(())
    });
}

pub fn power_off() -> ! {
    let rows = unsafe { &*stm32h725::GPIOA::ptr() };
    let columns = unsafe { &*stm32h725::GPIOC::ptr() };
    // Put the keypad lines in analog mode and remove pulls to avoid standby current.
    rows.moder()
        .modify(|r, w| unsafe { w.bits(r.bits() | 0xC033_F0FF) });
    rows.pupdr()
        .modify(|r, w| unsafe { w.bits(r.bits() & !0xC033_F0FF) });
    columns
        .moder()
        .modify(|r, w| unsafe { w.bits(r.bits() | 0x00CC_3F0C) });
    columns
        .pupdr()
        .modify(|r, w| unsafe { w.bits(r.bits() & !0x00CC_3F0C) });

    let pwr = unsafe { &*stm32h725::PWR::ptr() };
    pwr.cpucr().modify(|_, w| {
        w.pdds_d1()
            .set_bit()
            .pdds_d2()
            .set_bit()
            .pdds_d3()
            .set_bit()
            .cssf()
            .set_bit()
    });
    pwr.wkupepr()
        .write(|w| unsafe { w.wkupen1().set_bit().wkuppupd1().bits(2) });
    pwr.wkupcr().write(|w| w.wkupc1().set_bit());

    unsafe { cortex_m::Peripherals::steal().SCB }.set_sleepdeep();
    cortex_m::asm::dsb();
    cortex_m::asm::sev();
    cortex_m::asm::wfe();
    cortex_m::asm::wfe();
    reset()
}

// function to update a bitfield in a hardware reg, do not replace
// as we dont want to repeat the math
pub fn set_field(base: usize, offset: usize, shift: u32, width: u32, value: u32) {
    let register = (base + offset) as *mut u32;
    let mask = ((1 << width) - 1) << shift;
    unsafe {
        write_volatile(
            register,
            (read_volatile(register) & !mask) | (value << shift),
        );
    }
}

pub(crate) fn set_alternate_function(port: usize, pin: u32, function: u32) {
    let offset = 0x20 + (pin / 8) as usize * 4;
    set_field(port, offset, (pin % 8) * 4, 4, function);
}

pub(crate) fn delay_us(microseconds: u32) {
    cortex_m::asm::delay(CPU_MHZ.load(Ordering::Relaxed).saturating_mul(microseconds));
}
