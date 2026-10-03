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

pub fn scan_keyboard() -> u64 {
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

macro_rules! keys {
    ($($key:ident = $position:literal => $name:literal),+ $(,)?) => {
        #[repr(u8)]
        #[derive(Copy, Clone, Eq, PartialEq, Debug)]
        pub enum Key { $($key = $position),+ }

        impl Key {
            pub const ALL: &[Self] = &[$(Self::$key),+];

            pub const fn is_pressed(self, state: u64) -> bool {
                state & (1 << self as u8) != 0
            }

            pub const fn name(self) -> &'static str {
                match self { $(Self::$key => $name),+ }
            }
        }
    };
}

keys! {
    Left = 0 => "Left",
    Up = 1 => "Up",
    Down = 2 => "Down",
    Right = 3 => "Right",
    OK = 4 => "OK",
    Back = 5 => "Back",
    Home = 6 => "Home",
    OnOff = 8 => "OnOff",
    Shift = 12 => "Shift",
    Alpha = 13 => "Alpha",
    XNT = 14 => "XNT",
    Var = 15 => "Var",
    Toolbox = 16 => "Toolbox",
    Backspace = 17 => "Backspace",
    Exp = 18 => "Exp",
    Ln = 19 => "Ln",
    Log = 20 => "Log",
    Imaginary = 21 => "Imaginary",
    Comma = 22 => "Comma",
    Power = 23 => "Power",
    Sin = 24 => "Sin",
    Cos = 25 => "Cos",
    Tan = 26 => "Tan",
    Pi = 27 => "Pi",
    Sqrt = 28 => "Sqrt",
    Square = 29 => "Square",
    Seven = 30 => "7",
    Eight = 31 => "8",
    Nine = 32 => "9",
    LeftParenthesis = 33 => "LeftParenthesis",
    RightParenthesis = 34 => "RightParenthesis",
    Four = 36 => "4",
    Five = 37 => "5",
    Six = 38 => "6",
    Multiplication = 39 => "Multiplication",
    Division = 40 => "Division",
    One = 42 => "1",
    Two = 43 => "2",
    Three = 44 => "3",
    Plus = 45 => "Plus",
    Minus = 46 => "Minus",
    Zero = 48 => "0",
    Dot = 49 => "Dot",
    EE = 50 => "EE",
    Ans = 51 => "Ans",
    EXE = 52 => "EXE",
}
