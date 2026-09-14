// SPDX-License-Identifier: GPL-3.0-only

use core::ptr::{read_volatile, write_volatile};
use stm32h7::stm32h725;

const COMMAND_ADDRESS: usize = 0x6000_0000;
const DATA_ADDRESS: usize = 0x6002_0000;
const GPIOA: usize = 0x5802_0000;
const GPIOB: usize = 0x5802_0400;
const GPIOC: usize = 0x5802_0800;
const GPIOD: usize = 0x5802_0C00;
const GPIOE: usize = 0x5802_1000;
const DELAY_CPU_HZ: u32 = 550_000_000;
const WIDTH: u16 = 320;
const HEIGHT: u16 = 240;

// N0120 LCD bus pins as (GPIO port, pin, alternate function).
const FMC_PINS: [(usize, u32, u32); 20] = [
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
];

pub struct Display {
    _private: (),
}

impl Display {
    pub(crate) const fn new() -> Self {
        Self { _private: () }
    }

    pub fn initialize(&mut self) {
        let rcc = unsafe { &*stm32h725::RCC::ptr() };
        let fmc = unsafe { &*stm32h725::FMC::ptr() };

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
        for (port, pin, alternate_function) in FMC_PINS {
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

        delay_ms(120);

        // 16-bit asynchronous bus; read timing is 15/67 cycles, write is 6/6.
        fmc.bcr1().write(|w| unsafe { w.bits(0x0000_5010) });
        fmc.btr1().write(|w| unsafe { w.bits(0x0000_430F) });
        fmc.bwtr1().write(|w| unsafe { w.bits(0x0000_0606) });
        fmc.bcr1()
            .modify(|_, w| w.mbken().enabled().fmcen().enabled());

        cortex_m::asm::dsb();

        self.write_command(0x01); // Software reset
        delay_ms(5);

        self.write_command(0x11); // Sleep out
        delay_ms(120);

        self.write_command(0x3A); // 16-bit RGB565 pixels
        self.write_data(0x55);

        self.write_command(0x36); // Memory access control
        self.write_data(0xA0);

        self.write_command(0x21); // Display inversion on
        self.write_command(0x29); // Display on
        delay_ms(20);
    }

    pub fn fill_rgb565(&mut self, color: u16) {
        self.fill_rect(0, 0, WIDTH, HEIGHT, color);
    }

    pub fn fill_rect(&mut self, x: u16, y: u16, width: u16, height: u16, color: u16) {
        let x_end = x.saturating_add(width).min(WIDTH);
        let y_end = y.saturating_add(height).min(HEIGHT);
        if x >= x_end || y >= y_end {
            return;
        }

        self.write_command(0x2A); // Column range
        for value in [x >> 8, x & 0xFF, (x_end - 1) >> 8, (x_end - 1) & 0xFF] {
            self.write_data(value);
        }

        self.write_command(0x2B); // Row range
        for value in [y >> 8, y & 0xFF, (y_end - 1) >> 8, (y_end - 1) & 0xFF] {
            self.write_data(value);
        }

        self.write_command(0x2C); // Pixel data

        for _ in 0..u32::from(x_end - x) * u32::from(y_end - y) {
            self.write_data(color);
        }
    }

    fn write_command(&mut self, command: u8) {
        unsafe {
            write_volatile(COMMAND_ADDRESS as *mut u16, u16::from(command));
        }
    }

    fn write_data(&mut self, data: u16) {
        unsafe {
            write_volatile(DATA_ADDRESS as *mut u16, data);
        }
    }
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

fn delay_ms(milliseconds: u32) {
    cortex_m::asm::delay((DELAY_CPU_HZ / 1_000).saturating_mul(milliseconds));
}
