// SPDX-License-Identifier: GPL-3.0-only

use core::sync::atomic::{AtomicU32, Ordering};
use cortex_m::peripheral::NVIC;
use stm32h7::stm32h725::{self, Interrupt, interrupt};

use crate::mcu::ahb_frequency;

/// Times TIM2's 32-bit microsecond count has wrapped, about every 71 minutes.
static WRAPS: AtomicU32 = AtomicU32::new(0);

/// Starts TIM2 counting microseconds. Call after initialize_system.
pub fn initialize_timer() {
    let rcc = unsafe { &*stm32h725::RCC::ptr() };
    let timer = unsafe { &*stm32h725::TIM2::ptr() };
    rcc.apb1lenr().modify(|_, w| w.tim2en().enabled());
    let _ = rcc.apb1lenr().read().bits();
    // TIM2 runs at the AHB clock, since the APB1 prescaler isn't 1.
    timer
        .psc()
        .write(|w| unsafe { w.bits(ahb_frequency() / 1_000_000 - 1) });
    timer.arr().write(|w| unsafe { w.bits(u32::MAX) });
    timer.egr().write(|w| w.ug().set_bit()); // Load the prescaler now.
    timer.sr().write(|w| unsafe { w.bits(0) });
    timer.dier().write(|w| w.uie().set_bit());
    timer.cr1().write(|w| w.cen().set_bit());
    unsafe { NVIC::unmask(Interrupt::TIM2) };
}

pub fn micros() -> u64 {
    cortex_m::interrupt::free(|_| {
        let timer = unsafe { &*stm32h725::TIM2::ptr() };
        let mut low = timer.cnt().read().bits();
        // A set update flag is a wrap not counted yet.
        if timer.sr().read().uif().bit_is_set() {
            timer.sr().write(|w| unsafe { w.bits(!1) });
            WRAPS.fetch_add(1, Ordering::Relaxed);
            low = timer.cnt().read().bits();
        }
        u64::from(WRAPS.load(Ordering::Relaxed)) << 32 | u64::from(low)
    })
}

pub fn sleep_until(deadline: u64) {
    let timer = unsafe { &*stm32h725::TIM2::ptr() };
    // At most half a wrap ahead, so the compare match can't be missed.
    let wait = deadline.saturating_sub(micros()).min(1 << 31) as u32;
    if wait > 0 {
        timer
            .ccr1()
            .write(|w| unsafe { w.bits(timer.cnt().read().bits().wrapping_add(wait)) });
        timer.dier().modify(|_, w| w.cc1ie().set_bit());
        cortex_m::asm::wfi();
    }
}

#[interrupt]
fn TIM2() {
    let timer = unsafe { &*stm32h725::TIM2::ptr() };
    timer.sr().write(|w| unsafe { w.bits(!(1 << 1)) }); // Clear only the compare flag.
    micros();
}
