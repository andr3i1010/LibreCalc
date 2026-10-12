use crate::mcu::{
  GPIOB, 
  GPIOD, 
  GPIOJ,
  set_field,
  delay_us
};

pub fn initialize_spi() -> Result<(), &'static str> {
    let rcc = unsafe { &*stm32h7::stm32h725::RCC::ptr() };
    let ospi = unsafe { &*stm32h7::stm32h725::OCTOSPI1::ptr() };

    rcc.d1ccipr().modify(|_, w| w.octospisel().rcc_hclk3());
    rcc.ahb4enr().modify(|_, w| w.gpiojen().enabled());
    rcc.ahb3enr().modify(|_, w| w.octospi1en().enabled());
    let _ = rcc.ahb3enr().read().bits();

    // N0120 OCTOSPI1 pins, decoded from the stock board configuration.
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
        set_field(
            port,
            if pin < 8 { 0x20 } else { 0x24 },
            (pin % 8) * 4,
            4,
            alternate_function,
        );
    }

    let mut idle = false;
    for _ in 0..1_000_000 {
        if ospi.sr().read().bits() & (1 << 5) == 0 {
            idle = true;
            break;
        }
        cortex_m::asm::nop();
    }
    if !idle {
        return Err("OCTOSPI controller stayed busy");
    }

    ospi.cr().write(|w| unsafe { w.bits(0) });
    ospi.dcr1().write(|w| unsafe { w.bits(0x0016_0500) }); // 8 MiB flash.
    ospi.dcr2().write(|w| unsafe { w.bits(1) }); // Conservative 2:1 clock divider.
    ospi.cr().write(|w| unsafe { w.bits(1) });
    ospi.fcr().write(|w| unsafe { w.bits(0x1F) });
    ospi.dlr().write(|w| unsafe { w.bits(0) });
    ospi.ccr().write(|w| unsafe { w.bits(1) });
    ospi.ir().write(|w| unsafe { w.bits(0xAB) }); // Release deep power-down.

    let mut completed = false;
    for _ in 0..1_000_000 {
        let status = ospi.sr().read().bits();
        if status & 1 != 0 {
            return Err("OCTOSPI transfer error");
        }
        if status & 2 != 0 {
            completed = true;
            break;
        }
        cortex_m::asm::nop();
    }
    if !completed {
        return Err("OCTOSPI transfer timed out");
    }
    delay_us(3);

    // 0x0B fast read needs no flash status-register programming. A 24-bit
    // address covers both slots; eight dummy clocks precede the data.
    ospi.fcr().write(|w| unsafe { w.bits(0x1F) });
    ospi.cr().write(|w| unsafe { w.bits(0x3000_0001) });
    ospi.dlr().write(|w| unsafe { w.bits(0x007F_FFFF) });
    ospi.tcr().write(|w| unsafe { w.bits(8) });
    ospi.ccr().write(|w| unsafe { w.bits(0x0100_2101) });
    ospi.ar().write(|w| unsafe { w.bits(0) });
    ospi.ir().write(|w| unsafe { w.bits(0x0B) });
    cortex_m::asm::dsb();
    cortex_m::asm::isb();
    Ok(())
}