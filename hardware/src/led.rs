// SPDX-License-Identifier: GPL-3.0-only

use embedded_graphics::{pixelcolor::Rgb565, prelude::IntoStorage};

use crate::mcu::{set_field, GPIOB, GPIOC};

const PERIOD: u16 = 58;

pub struct Led {
    enabled: bool,
}

impl Led {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let rcc = unsafe { &*stm32h7::stm32h725::RCC::ptr() };
        let timer = unsafe { &*stm32h7::stm32h725::TIM3::ptr() };

        rcc.ahb4enr()
            .modify(|_, w| w.gpioben().enabled().gpiocen().enabled());
        rcc.apb1lenr().modify(|_, w| w.tim3en().enabled());
        let _ = rcc.apb1lenr().read().bits();
        rcc.apb1lrstr().modify(|_, w| w.tim3rst().set_bit());
        rcc.apb1lrstr().modify(|_, w| w.tim3rst().clear_bit());

        timer.psc().write(|w| unsafe { w.bits(2047) });
        timer
            .arr()
            .write(|w| unsafe { w.bits(u32::from(PERIOD - 1)) });
        timer
            .ccmr1_output()
            .write(|w| w.oc1m().pwm_mode2().oc2m().pwm_mode2());
        timer.ccmr2_output().write(|w| w.oc3m().pwm_mode2());
        for channel in 0..3 {
            timer
                .ccr(channel)
                .write(|w| unsafe { w.bits(u32::from(PERIOD)) });
        }
        timer.egr().write(|w| w.ug().update());
        timer.sr().write(|w| unsafe { w.bits(0) });

        for (port, pin) in [(GPIOB, 4), (GPIOB, 5), (GPIOC, 8)] {
            set_field(port, 0x04, pin, 1, 0);
            set_field(port, 0x08, pin * 2, 2, 0);
            set_field(port, 0x0C, pin * 2, 2, 0);
            set_field(port, if pin < 8 { 0x20 } else { 0x24 }, (pin % 8) * 4, 4, 2);
            set_field(port, 0x00, pin * 2, 2, 3);
        }

        Self { enabled: false }
    }

    pub fn set_color(&mut self, color: Rgb565) {
        let bits = color.into_storage();
        let timer = unsafe { &*stm32h7::stm32h725::TIM3::ptr() };
        if bits == 0 {
            if self.enabled {
                timer.ccer().write(|w| unsafe { w.bits(0) });
                timer.cr1().write(|w| w.cen().disabled());
                for (port, pin) in [(GPIOB, 4), (GPIOB, 5), (GPIOC, 8)] {
                    set_field(port, 0x00, pin * 2, 2, 3);
                }
                self.enabled = false;
            }
            return;
        }

        for (channel, component, maximum) in [
            (0, (bits >> 11) & 31, 31),
            (1, (bits >> 5) & 63, 63),
            (2, bits & 31, 31),
        ] {
            let compare = PERIOD - component * PERIOD / maximum;
            timer
                .ccr(channel)
                .write(|w| unsafe { w.bits(u32::from(compare)) });
        }
        if !self.enabled {
            for (port, pin) in [(GPIOB, 4), (GPIOB, 5), (GPIOC, 8)] {
                set_field(port, 0x00, pin * 2, 2, 2);
            }
            timer
                .ccer()
                .write(|w| w.cc1e().enabled().cc2e().enabled().cc3e().enabled());
            timer.cr1().write(|w| w.cen().enabled());
            self.enabled = true;
        }
    }
}
