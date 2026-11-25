// SPDX-License-Identifier: GPL-3.0-only
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use librecalc_abi::{Error, Rect, setting};

librecalc_abi::app!("Hello", main);

fn main() -> u32 {
    let mut buffer = [0; 16];
    if librecalc_abi::os_version(&mut buffer).is_err() {
        return 1;
    }
    let kernel_ram = unsafe { core::slice::from_raw_parts_mut(0x2000_0000 as *mut u8, 16) };
    if librecalc_abi::os_version(kernel_ram) != Err(Error::EFAULT) {
        return 2;
    }
    let Ok(page) = librecalc_abi::alloc(4096) else {
        return 3;
    };
    unsafe { (page as *mut u32).write_volatile(0xC0FFEE) };
    if unsafe { (page as *const u32).read_volatile() } != 0xC0FFEE
        || unsafe { librecalc_abi::free(page) }.is_err()
    {
        return 4;
    }
    let numbers: Vec<u32> = (0..100).collect();
    if numbers.iter().sum::<u32>() != 4950 {
        return 5;
    }
    let screen = Rect {
        x: 0,
        y: 0,
        width: 320,
        height: 240,
    };
    const SIZE: u16 = 32;
    static SQUARE: [u16; SIZE as usize * SIZE as usize] = {
        let mut pixels = [0; SIZE as usize * SIZE as usize];
        let mut index = 0;
        while index < pixels.len() {
            let red = 31 - (index % SIZE as usize) as u16;
            pixels[index] = red << 11 | (31 - red);
            index += 1;
        }
        pixels
    };
    let light = librecalc_abi::setting(setting::THEME) == Ok(1);
    let background = if light { 0xFFFF } else { 0x0000 };
    librecalc_abi::display_fill_rect(screen, background).unwrap();
    let start = librecalc_abi::uptime().unwrap();
    while librecalc_abi::keyboard_scan() == Ok(0) {
        let x =
            ((librecalc_abi::uptime().unwrap() - start) / 10_000 % u64::from(320 - SIZE)) as u16;
        let square = Rect {
            x,
            y: 104,
            width: SIZE,
            height: SIZE,
        };
        librecalc_abi::display_wait_for_vblank().unwrap();
        let right = x + SIZE;
        librecalc_abi::display_fill_rect(
            Rect {
                x: 0,
                width: x,
                ..square
            },
            background,
        )
        .unwrap();
        librecalc_abi::display_fill_rect(
            Rect {
                x: right,
                width: 320 - right,
                ..square
            },
            background,
        )
        .unwrap();
        librecalc_abi::display_push_rect(square, &SQUARE).unwrap();
    }
    librecalc_abi::set_setting(setting::THEME, u32::from(!light)).unwrap();
    librecalc_abi::sleep_us(200_000).unwrap();
    #[cfg(feature = "crash")]
    unsafe {
        (0x2000_0000 as *const u32).read_volatile();
    }
    0
}
