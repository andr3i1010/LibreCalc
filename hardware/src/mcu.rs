// SPDX-License-Identifier: GPL-3.0-only

use core::ptr::{read_volatile, write_volatile};

const GPIOA: usize = 0x5802_0000;
const GPIOB: usize = 0x5802_0400;
const GPIOC: usize = 0x5802_0800;
const GPIOD: usize = 0x5802_0C00;
const GPIOE: usize = 0x5802_1000;
const BACKLIGHT_PIN: u32 = 3;

pub fn initialize_system() {
    let device = stm32h7::stm32h725::Peripherals::take().unwrap();
    let flash = &device.FLASH;
    let fmc = &device.FMC;
    let pwr = &device.PWR;
    let rcc = &device.RCC;
    let syscfg = &device.SYSCFG;

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
    while flash.acr().read().latency().bits() != 3 || flash.acr().read().wrhighfreq().bits() != 3 {}

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

    rcc.d1ccipr().modify(|_, w| w.fmcsel().rcc_hclk3());
    rcc.apb4enr().modify(|_, w| w.syscfgen().enabled());
    let _ = rcc.apb4enr().read().bits();
    syscfg.cccsr().modify(|_, w| w.en().set_bit());
    while syscfg.cccsr().read().ready().bit_is_clear() {}

    // FMC LCD command/data writes must be ordered device accesses.
    // Match the stock 256 MiB region: execute-never, privileged RW, TEX=2.
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

    // Select medium-speed alternate-function mode for every FMC signal.
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

    // Board-level LCD control lines; stock startup drives all three high.
    set_field(GPIOE, 0x00, 4 * 2, 2, 1);
    set_field(GPIOE, 0x00, 5 * 2, 2, 1);
    set_field(GPIOC, 0x00, 13 * 2, 2, 1);
    unsafe {
        write_volatile((GPIOE + 0x18) as *mut u32, (1 << 4) | (1 << 5));
        write_volatile((GPIOC + 0x18) as *mut u32, 1 << 13);
    }

    // PB1 receives the LCD tearing-effect signal.
    set_field(GPIOB, 0x08, 2, 2, 1);
    set_field(GPIOB, 0x00, 2, 2, 0);
    set_field(GPIOB, 0x0C, 2, 2, 0);

    // 16-bit asynchronous bus; read timing is 15/67 cycles, write is 6/6.
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

fn set_field(base: usize, offset: usize, shift: u32, width: u32, value: u32) {
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
