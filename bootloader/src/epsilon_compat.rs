// SPDX-License-Identifier: GPL-3.0-only

use core::ptr::{read_volatile, write_volatile};

#[repr(C)]
struct Trampoline {
    suspend: extern "C" fn(),
    erase_sector: extern "C" fn(u32),
    write_memory: extern "C" fn(*mut u8, *const u8, usize),
    memcmp: extern "C" fn(*const u8, *const u8, usize) -> i32,
    memcpy: extern "C" fn(*mut u8, *const u8, usize) -> *mut u8,
    memmove: extern "C" fn(*mut u8, *const u8, usize) -> *mut u8,
    memset: extern "C" fn(*mut u8, i32, usize) -> *mut u8,
    strchr: extern "C" fn(*const u8, i32) -> *const u8,
    strcmp: extern "C" fn(*const u8, *const u8) -> i32,
    strlcat: extern "C" fn(*mut u8, *const u8, usize) -> usize,
    strlcpy: extern "C" fn(*mut u8, *const u8, usize) -> usize,
    strlen: extern "C" fn(*const u8) -> usize,
    strncmp: extern "C" fn(*const u8, *const u8, usize) -> i32,
}

#[used]
#[unsafe(link_section = ".trampoline")]
static TRAMPOLINE: Trampoline = Trampoline {
    suspend,
    erase_sector,
    write_memory,
    memcmp,
    memcpy,
    memmove,
    memset,
    strchr,
    strcmp,
    strlcat,
    strlcpy,
    strlen,
    strncmp,
};

#[used]
#[unsafe(link_section = ".epsilon_keypad_rows")]
static KEYPAD_ROWS: [u8; 9] = [1, 0, 2, 3, 6, 7, 8, 10, 15];

#[used]
#[unsafe(link_section = ".epsilon_keypad_columns")]
static KEYPAD_COLUMNS: [u8; 12] = [1, 2, 3, 4, 5, 6, 1, 9, 11, 4, 5, 6];

extern "C" fn suspend() {
    #[cfg(target_arch = "arm")]
    unsafe {
        let wakeup_clear = 0x5802_4820 as *mut u32;
        let wakeup_enable = 0x5802_4828 as *mut u32;
        let wakeup = read_volatile(wakeup_enable);
        write_volatile(wakeup_enable, (wakeup & !0x0003_0101) | 0x0002_0001);
        write_volatile(wakeup_clear, 1);
        let scr = 0xE000_ED10 as *mut u32;
        write_volatile(scr, read_volatile(scr) & !(1 << 2));
        core::arch::asm!("dsb sy", "sev", "wfe", "wfe", options(nostack));
        write_volatile(wakeup_clear, 1);
        let rcc_apb1_clock_enable = 0x5802_44E8 as *mut u32;
        write_volatile(
            rcc_apb1_clock_enable,
            read_volatile(rcc_apb1_clock_enable) | 1,
        );

        let timer = 0x4000_0000 as *mut u32;
        write_volatile(timer.add(10), 53_999);
        write_volatile(timer.add(11), 2_500);
        write_volatile(timer.add(5), 1);
        write_volatile(timer.add(4), read_volatile(timer.add(4)) & !1);
        write_volatile(timer.add(3), read_volatile(timer.add(3)) | 1);
        write_volatile(timer, read_volatile(timer) | 5);

        write_volatile((0xE000_E400usize + 28) as *mut u8, 0x70);
        write_volatile(0xE000_E280 as *mut u32, 1 << 28);
        for (bank, mask) in [0x5080_07C0, 0x0040_0100, 0, 0x8000_0000]
            .into_iter()
            .enumerate()
        {
            write_volatile((0xE000_E100 as *mut u32).add(bank), mask);
        }

        write_volatile(0xE000_E014 as *mut u32, 0x0001_07AB);
        write_volatile(0xE000_E018 as *mut u32, 0);
        write_volatile(0xE000_E010 as *mut u32, 7);
        core::arch::asm!("dsb sy", "isb sy", options(nostack));
    }
}

// no numworks were not replacing our SUPERIOR bootloader 😘😘😘
extern "C" fn erase_sector(_sector: u32) {}

extern "C" fn write_memory(_destination: *mut u8, _source: *const u8, _length: usize) {}

extern "C" fn memcmp(a: *const u8, b: *const u8, length: usize) -> i32 {
    for i in 0..length {
        let x = unsafe { read_volatile(a.add(i)) };
        let y = unsafe { read_volatile(b.add(i)) };
        if x != y {
            return i32::from(x) - i32::from(y);
        }
    }
    0
}

extern "C" fn memcpy(destination: *mut u8, source: *const u8, length: usize) -> *mut u8 {
    for i in 0..length {
        unsafe { write_volatile(destination.add(i), read_volatile(source.add(i))) };
    }
    destination
}

extern "C" fn memmove(destination: *mut u8, source: *const u8, length: usize) -> *mut u8 {
    if (destination as usize).wrapping_sub(source as usize) < length {
        for i in (0..length).rev() {
            unsafe { write_volatile(destination.add(i), read_volatile(source.add(i))) };
        }
    } else {
        memcpy(destination, source, length);
    }
    destination
}

extern "C" fn memset(destination: *mut u8, value: i32, length: usize) -> *mut u8 {
    for i in 0..length {
        unsafe { write_volatile(destination.add(i), value as u8) };
    }
    destination
}

extern "C" fn strchr(string: *const u8, character: i32) -> *const u8 {
    let mut offset = 0;
    loop {
        let current = unsafe { read_volatile(string.add(offset)) };
        if current == character as u8 {
            return unsafe { string.add(offset) };
        }
        if current == 0 {
            return core::ptr::null();
        }
        offset += 1;
    }
}

extern "C" fn strcmp(a: *const u8, b: *const u8) -> i32 {
    strncmp(a, b, usize::MAX)
}

extern "C" fn strlcat(destination: *mut u8, source: *const u8, capacity: usize) -> usize {
    let mut used = 0;
    while used < capacity && unsafe { read_volatile(destination.add(used)) } != 0 {
        used += 1;
    }
    if used == capacity {
        return capacity + strlen(source);
    }
    used + strlcpy(unsafe { destination.add(used) }, source, capacity - used)
}

extern "C" fn strlcpy(destination: *mut u8, source: *const u8, capacity: usize) -> usize {
    let source_length = strlen(source);
    if capacity != 0 {
        let copy = source_length.min(capacity - 1);
        memcpy(destination, source, copy);
        unsafe { write_volatile(destination.add(copy), 0) };
    }
    source_length
}

extern "C" fn strlen(string: *const u8) -> usize {
    let mut length = 0;
    while unsafe { read_volatile(string.add(length)) } != 0 {
        length += 1;
    }
    length
}

extern "C" fn strncmp(a: *const u8, b: *const u8, length: usize) -> i32 {
    for i in 0..length {
        let x = unsafe { read_volatile(a.add(i)) };
        let y = unsafe { read_volatile(b.add(i)) };
        if x != y || x == 0 {
            return i32::from(x) - i32::from(y);
        }
    }
    0
}
