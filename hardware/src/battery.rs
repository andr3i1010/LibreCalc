// SPDX-License-Identifier: GPL-3.0-only

use stm32h7::stm32h725;

use crate::mcu::{GPIOB, GPIOC, delay_us, set_field};

const TIMEOUT: usize = 1_000_000;

pub fn initialize_battery() -> Result<(), &'static str> {
    let rcc = unsafe { &*stm32h725::RCC::ptr() };
    let adc = unsafe { &*stm32h725::ADC1::ptr() };
    let common = unsafe { &*stm32h725::ADC12_COMMON::ptr() };
    let reference = unsafe { &*stm32h725::VREFBUF::ptr() };

    rcc.ahb4enr()
        .modify(|_, w| w.gpioben().enabled().gpiocen().enabled());
    rcc.apb4enr()
        .modify(|_, w| w.syscfgen().enabled().vrefen().enabled());
    let _ = rcc.apb4enr().read().bits();
    rcc.ahb1enr().modify(|_, w| w.adc12en().enabled());
    let _ = rcc.ahb1enr().read().bits();
    set_field(GPIOB, 0x00, 0, 2, 3);
    set_field(GPIOB, 0x0C, 0, 2, 0);
    set_field(GPIOC, 0x00, 20, 2, 0);
    set_field(GPIOC, 0x0C, 20, 2, 1);

    // Rescue firmware may have already calibrated and enabled the battery ADC.
    if adc.cr().read().aden().bit_is_set() {
        return battery_voltage().map(|_| ());
    }

    // A fixed CSI clock keeps conversions independent of CPU speed and PLL2.
    rcc.cr().modify(|_, w| w.csion().set_bit());
    if !(0..TIMEOUT).any(|_| rcc.cr().read().csirdy().bit_is_set()) {
        return Err("battery ADC clock timeout");
    }
    rcc.d1ccipr().modify(|_, w| w.ckpersel().csi());
    rcc.d3ccipr().modify(|_, w| w.adcsel().per());
    common.ccr().write(|w| unsafe { w.bits(0) });

    reference
        .csr()
        .write(|w| unsafe { w.envr().set_bit().hiz().clear_bit().vrs().bits(0) });
    if !(0..TIMEOUT).any(|_| reference.csr().read().vrr().bit_is_set()) {
        return Err("battery voltage reference timeout");
    }

    adc.cr().modify(|_, w| w.deeppwd().power_up());
    adc.cr()
        .modify(|_, w| w.advregen().enabled().boost().lt6_25());
    if !(0..TIMEOUT).any(|_| adc.isr().read().ldordy().bit_is_set()) {
        return Err("battery ADC regulator timeout");
    }
    for differential in [false, true] {
        adc.cr()
            .modify(|_, w| w.adcaldif().bit(differential).adcallin().bit(!differential));
        adc.cr().modify(|_, w| w.adcal().start_calibration());
        // Linearity calibration can take 165010 ADC cycles, about 42 ms at 4 MHz.
        if !(0..32 * TIMEOUT).any(|_| adc.cr().read().adcal().bit_is_clear()) {
            return Err("battery ADC calibration timeout");
        }
    }
    delay_us(1); // Allow four ADC clock cycles after calibration before setting ADEN.

    adc.cfgr().write(|w| unsafe { w.bits(0) });
    adc.pcsel().modify(|_, w| w.pcsel9().preselected());
    adc.smpr1().modify(|_, w| w.smp9().cycles810_5());
    adc.sqr1().write(|w| unsafe { w.sq1().bits(9) });
    adc.isr().write(|w| w.adrdy().clear());
    adc.cr().modify(|_, w| w.aden().enabled());
    if !(0..TIMEOUT).any(|_| adc.isr().read().adrdy().bit_is_set()) {
        return Err("battery ADC enable timeout");
    }
    battery_voltage().map(|_| ())
}

pub fn battery_voltage() -> Result<f32, &'static str> {
    let adc = unsafe { &*stm32h725::ADC1::ptr() };
    if adc.cr().read().aden().bit_is_clear() {
        return Err("battery ADC is not initialized");
    }
    if adc.cr().read().adstart().bit_is_set() {
        return Err("battery ADC is busy");
    }
    adc.isr()
        .write(|w| w.eoc().clear().eos().clear().ovr().clear());
    adc.cr().modify(|_, w| w.adstart().start_conversion());
    if !(0..TIMEOUT).any(|_| adc.isr().read().eoc().bit_is_set()) {
        return Err("battery ADC conversion timeout");
    }
    // The board uses a 2.5 V reference and a 2:1 battery voltage divider.
    Ok(adc.dr().read().rdata().bits() as f32 * 5.0 / 65_535.0)
}

pub fn battery_is_charging() -> bool {
    let port = unsafe { &*stm32h725::GPIOC::ptr() };
    port.idr().read().bits() & (1 << 10) == 0
}

/// Voltage-based estimate using Omicron's battery percentage calculation.
pub fn battery_percentage() -> Result<u8, &'static str> {
    Ok(((battery_voltage()? - 3.6) * 166.0).clamp(0.0, 100.0) as u8)
}
