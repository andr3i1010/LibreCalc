// SPDX-License-Identifier: GPL-3.0-only
use core::cell::Cell;
use cortex_m::interrupt::Mutex;
use librecalc_abi::{Error, Key};

/// The keys at the last scan, SHIFT for the next key, and ALPHA: 1 for the next key, 2 locked.
static STATE: Mutex<Cell<(u64, bool, u8)>> = Mutex::new(Cell::new((0, false, 0)));

const FORCE_CLOSE: u64 = 1 << Key::Shift as u8 | 1 << Key::Alpha as u8 | 1 << Key::Home as u8;


pub fn scan(owner: u32, next: &mut Option<usize>) -> u64 {
    let mut keys = n0120_hardware::read_keyboard();
    if Key::OnOff.is_pressed(keys) {
        n0120_hardware::standby();
        keys = n0120_hardware::read_keyboard();
    }
    let force_close = cortex_m::interrupt::free(|cs| {
        let state = STATE.borrow(cs);
        let (previous, mut shift, mut alpha) = state.get();
        let pressed = keys & !previous;
        if pressed >> (Key::Alpha as u8 + 1) != 0 {
            (shift, alpha) = (false, alpha & 2);
        }
        shift ^= Key::Shift.is_pressed(pressed);
        alpha = (alpha + u8::from(Key::Alpha.is_pressed(pressed))) % 3;
        let force_close = keys & FORCE_CLOSE == FORCE_CLOSE && pressed & FORCE_CLOSE != 0;
        if force_close {
            (shift, alpha) = (false, 0);
        }
        state.set((keys, shift, alpha));
        force_close
    });
    if force_close {
        *next = Some(crate::process::exit(owner, Err(Error::EINTR)));
    }
    keys
}

pub fn modifiers() -> u32 {
    let (_, shift, alpha) = cortex_m::interrupt::free(|cs| STATE.borrow(cs).get());
    u32::from(shift) | u32::from(alpha) << 1
}
