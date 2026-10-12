// SPDX-License-Identifier: GPL-3.0-only

use core::ptr::{read_volatile, write_volatile};

pub const GPIOA: usize = 0x5802_0000;
pub const GPIOB: usize = 0x5802_0400;
pub const GPIOC: usize = 0x5802_0800;
pub const GPIOD: usize = 0x5802_0C00;
pub const GPIOE: usize = 0x5802_1000;
pub const GPIOJ: usize = 0x5802_2400;
pub const BACKLIGHT_PIN: u32 = 3;

// here comes the ai slop, hope you (don't) enjoy
// yall really think a 14 year old can reverse engineer an entire proprietary system on their own?

pub fn initialize_system() {
    let device = stm32h7::stm32h725::Peripherals::take().unwrap();
    let flash = &device.FLASH;
    let fmc = &device.FMC;
    let pwr = &device.PWR;
    let rcc = &device.RCC;
    let syscfg = &device.SYSCFG;

    // if the bootloader already set the clock, preserve it instead of weakening it more
    let clock_configured = rcc.cfgr().read().sw().is_pll1()
        && rcc.cfgr().read().sws().is_pll1()
        && rcc.cr().read().csirdy().bit_is_set()
        && rcc.cr().read().pll1on().bit_is_set()
        && rcc.cr().read().pll1rdy().bit_is_set()
        && rcc.pllckselr().read().pllsrc().is_csi()
        && rcc.pllckselr().read().divm1().bits() == 4
        && rcc.pllcfgr().read().pll1fracen().is_reset()
        && rcc.pllcfgr().read().pll1vcosel().is_wide_vco()
        && rcc.pllcfgr().read().pll1rge().is_range1()
        && rcc.pllcfgr().read().divp1en().is_enabled()
        && rcc.pllcfgr().read().divq1en().is_disabled()
        && rcc.pllcfgr().read().divr1en().is_disabled()
        && rcc.pll1fracr().read().fracn1().bits() == 0
        && rcc.pll1divr().read().divn1().bits() == 269
        && rcc.pll1divr().read().divp1().is_div1()
        && rcc.d1cfgr().read().d1cpre().is_div1()
        && rcc.d1cfgr().read().hpre().is_div2()
        && rcc.d1cfgr().read().d1ppre().is_div2()
        && rcc.d2cfgr().read().d2ppre1().is_div2()
        && rcc.d2cfgr().read().d2ppre2().is_div2()
        && rcc.d3cfgr().read().d3ppre().is_div2()
        && pwr.cr3().read().bypass().bit_is_clear()
        && pwr.cr3().read().ldoen().bit_is_clear()
        && pwr.cr3().read().sden().bit_is_set()
        && pwr.cr3().read().sdexthp().bit_is_clear()
        && pwr.cr3().read().sdlevel().bits() == 0
        && pwr.csr1().read().actvosrdy().bit_is_set()
        && pwr.d3cr().read().vos().bits() == 0
        && pwr.d3cr().read().vosrdy().bit_is_set()
        && flash.acr().read().latency().bits() == 3
        && flash.acr().read().wrhighfreq().bits() == 3;

    if !clock_configured {
        rcc.cr().modify(|_, w| w.hsion().set_bit());
        while rcc.cr().read().hsirdy().bit_is_clear() {}
        rcc.cfgr().modify(|_, w| w.sw().hsi());
        while !rcc.cfgr().read().sws().is_hsi() {}

        rcc.cr().modify(|_, w| w.pll1on().clear_bit());
        while rcc.cr().read().pll1rdy().bit_is_set() {}

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
        while pwr.csr1().read().actvosrdy().bit_is_clear() {}

        pwr.d3cr().modify(|_, w| unsafe { w.vos().bits(0) });
        while pwr.d3cr().read().vosrdy().bit_is_clear() {}

        flash
            .acr()
            .modify(|_, w| unsafe { w.latency().bits(3).wrhighfreq().bits(3) });
        while flash.acr().read().latency().bits() != 3
            || flash.acr().read().wrhighfreq().bits() != 3
        {}

        rcc.cr().modify(|_, w| w.csion().set_bit());
        while rcc.cr().read().csirdy().bit_is_clear() {}
        rcc.pllckselr()
            .modify(|_, w| unsafe { w.pllsrc().csi().divm1().bits(4) });
        rcc.pllcfgr().modify(|_, w| {
            w.pll1fracen()
                .reset()
                .pll1vcosel()
                .wide_vco()
                .pll1rge()
                .range1()
                .divp1en()
                .enabled()
                .divq1en()
                .disabled()
                .divr1en()
                .disabled()
        });
        rcc.pll1divr()
            .modify(|_, w| unsafe { w.divn1().bits(269).divp1().div1() });

        rcc.d1cfgr()
            .modify(|_, w| w.d1cpre().div1().hpre().div2().d1ppre().div2());
        rcc.d2cfgr()
            .modify(|_, w| w.d2ppre1().div2().d2ppre2().div2());
        rcc.d3cfgr().modify(|_, w| w.d3ppre().div2());

        rcc.cr().modify(|_, w| w.pll1on().set_bit());
        while rcc.cr().read().pll1rdy().bit_is_clear() {}
        rcc.cfgr().modify(|_, w| w.sw().pll1());
        while !rcc.cfgr().read().sws().is_pll1() {}
    }

    rcc.d1ccipr().modify(|_, w| w.fmcsel().rcc_hclk3());
    rcc.apb4enr().modify(|_, w| w.syscfgen().enabled());
    let _ = rcc.apb4enr().read().bits();
    syscfg.cccsr().modify(|_, w| w.en().set_bit());
    while syscfg.cccsr().read().ready().bit_is_clear() {}

    let mpu = &cortex_m::Peripherals::take().unwrap().MPU;
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
    let _ = rcc.ahb4enr().read().bits();

    for (port, pin, alternate_function) in [
        (GPIOA, 4, 12),
        (GPIOA, 5, 12),
        (GPIOB, 14, 12),
        (GPIOB, 15, 12),
        (GPIOC, 0, 1),
        (GPIOC, 7, 9),
        (GPIOC, 12, 1),
        (GPIOD, 0, 12),
        (GPIOD, 1, 12),
        (GPIOD, 2, 1),
        (GPIOD, 4, 12),
        (GPIOD, 5, 12),
        (GPIOD, 8, 12),
        (GPIOD, 9, 12),
        (GPIOD, 10, 12),
        (GPIOD, 11, 12),
        (GPIOD, 14, 12),
        (GPIOD, 15, 12),
        (GPIOE, 7, 12),
        (GPIOE, 8, 12),
    ] {
        set_field(port, 0x08, pin * 2, 2, 1);
        set_field(port, 0x00, pin * 2, 2, 2);
        set_field(
            port,
            if pin < 8 { 0x20 } else { 0x24 },
            (pin % 8) * 4,
            4,
            alternate_function,
        );
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

pub fn reset() -> ! {
    cortex_m::peripheral::SCB::sys_reset()
}

pub fn power_off() -> ! {
    let rows = unsafe { &*stm32h7::stm32h725::GPIOA::ptr() };
    let columns = unsafe { &*stm32h7::stm32h725::GPIOC::ptr() };
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

    let pwr = unsafe { &*stm32h7::stm32h725::PWR::ptr() };
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
    pwr.wkupepr().write(|w| unsafe {
        w.wkupen1()
            .set_bit()
            .wkupp1()
            .clear_bit()
            .wkuppupd1()
            .bits(2)
    });
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

pub(crate) fn delay_us(microseconds: u32) {
    cortex_m::asm::delay(270u32.saturating_mul(microseconds));
}
