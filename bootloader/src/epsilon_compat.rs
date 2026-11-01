// SPDX-License-Identifier: GPL-3.0-only

use core::ptr::{read_volatile, write_volatile};
use n0120_hardware::{
    EXTERNAL_FLASH_END, EXTERNAL_FLASH_START, INTERNAL_FLASH_END, INTERNAL_FLASH_START,
    erase_external_flash, program_external_flash, suspend,
};

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
#[unsafe(link_section = ".epsilon_keypad_rows")]
static KEYPAD_ROWS: [u8; 9] = [1, 0, 2, 3, 6, 7, 8, 10, 15];

#[used]
#[unsafe(link_section = ".epsilon_keypad_columns")]
static KEYPAD_COLUMNS: [u8; 12] = [1, 2, 3, 4, 5, 6, 1, 9, 11, 4, 5, 6];

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

extern "C" fn erase_sector(sector: u32) {
    let offset = match sector {
        0..=7 => sector * 0x1000,
        8 => 0x8000,
        9..=135 => (sector - 8) * 0x1_0000,
        _ => return,
    };
    let address = EXTERNAL_FLASH_START + offset;
    let _ = erase_external_flash::<false>(address);
}

extern "C" fn write_memory(destination: *mut u8, source: *const u8, length: usize) {
    let address = destination as u32;
    let Some(end) = address.checked_add(length as u32) else {
        return;
    };
    let Some(source_end) = (source as u32).checked_add(length as u32) else {
        return;
    };
    if length == 0
        || address < EXTERNAL_FLASH_START
        || end > EXTERNAL_FLASH_END
        || ![
            (INTERNAL_FLASH_START, INTERNAL_FLASH_END),
            (EXTERNAL_FLASH_START, EXTERNAL_FLASH_END),
            (0x2000_0000, 0x2002_0000),
            (0x2400_0000, 0x2405_0000),
        ]
        .into_iter()
        .any(|(start, end)| source as u32 >= start && source_end <= end)
    {
        return;
    }

    let data = unsafe { core::slice::from_raw_parts(source, length) };
    let _ = program_external_flash::<false>(address, data);
}

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
