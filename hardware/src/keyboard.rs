// SPDX-License-Identifier: GPL-3.0-only

use stm32h7::stm32h725;

use crate::mcu::delay_us;

const COLUMNS: [u8; 6] = [1, 9, 11, 4, 5, 6];

pub fn initialize_keyboard() {
    let rows = unsafe { &*stm32h725::GPIOA::ptr() };
    let columns = unsafe { &*stm32h725::GPIOC::ptr() };

    rows.bsrr().write(|w| unsafe { w.bits(0x85CF) });
    rows.otyper()
        .modify(|r, w| unsafe { w.bits(r.bits() | 0x85CF) });
    rows.pupdr()
        .modify(|r, w| unsafe { w.bits(r.bits() & !0xC033_F0FF) });
    rows.moder()
        .modify(|r, w| unsafe { w.bits((r.bits() & !0xC033_F0FF) | 0x4011_5055) });
    columns
        .pupdr()
        .modify(|r, w| unsafe { w.bits((r.bits() & !0x00CC_3F0C) | 0x0044_1504) });
    columns
        .moder()
        .modify(|r, w| unsafe { w.bits(r.bits() & !0x00CC_3F0C) });
}

pub fn read_keyboard() -> u64 {
    let rows = unsafe { &*stm32h725::GPIOA::ptr() };
    let columns = unsafe { &*stm32h725::GPIOC::ptr() };
    let mut bits = 0;

    for (row, pin) in [1, 0, 2, 3, 6, 7, 8, 10, 15].into_iter().enumerate() {
        rows.bsrr().write(|w| unsafe { w.bits(1 << (pin + 16)) });
        delay_us(100);
        let input = columns.idr().read().bits();
        for (column, pin) in COLUMNS.into_iter().enumerate() {
            if input & (1 << pin) == 0 {
                bits |= 1 << (row * COLUMNS.len() + column);
            }
        }
        rows.bsrr().write(|w| unsafe { w.bits(1 << pin) });
    }

    bits
}

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum Key {
    Left = 0,
    Up = 1,
    Down = 2,
    Right = 3,
    OK = 4,
    Back = 5,
    Home = 6,
    OnOff = 8,
    Shift = 12,
    Alpha = 13,
    XNT = 14,
    Var = 15,
    Toolbox = 16,
    Backspace = 17,
    Exp = 18,
    Ln = 19,
    Log = 20,
    Imaginary = 21,
    Comma = 22,
    Power = 23,
    Sin = 24,
    Cos = 25,
    Tan = 26,
    Pi = 27,
    Sqrt = 28,
    Square = 29,
    Seven = 30,
    Eight = 31,
    Nine = 32,
    LeftParenthesis = 33,
    RightParenthesis = 34,
    Four = 36,
    Five = 37,
    Six = 38,
    Multiplication = 39,
    Division = 40,
    One = 42,
    Two = 43,
    Three = 44,
    Plus = 45,
    Minus = 46,
    Zero = 48,
    Dot = 49,
    EE = 50,
    Ans = 51,
    EXE = 52,
}

impl Key {
    pub const fn is_pressed(self, state: u64) -> bool {
        state & (1u64 << self as u8) != 0
    }
}
