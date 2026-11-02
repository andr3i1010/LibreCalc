// SPDX-License-Identifier: GPL-3.0-only

use crate::Error;
use core::alloc::{GlobalAlloc, Layout};

/// Install as the app's global allocator; ordinary infallible Rust allocations
/// retain Rust's OOM behavior. Syscall code uses fallible allocation.
pub struct AppAllocator;

unsafe impl GlobalAlloc for AppAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe {
            allocate(layout, |size| {
                crate::memory_alloc(size).map(|address| address as usize)
            })
        }
    }

    unsafe fn dealloc(&self, address: *mut u8, _: Layout) {
        let _ = unsafe { crate::memory_free(allocation_base(address) as u32) };
    }
}

/// # Safety
/// The backend must return exclusive writable storage of at least the requested
/// size, or an error. Storage must remain valid until the matching free.
pub unsafe fn allocate(
    layout: Layout,
    backend: impl FnOnce(u32) -> Result<usize, Error>,
) -> *mut u8 {
    let alignment = layout.align().max(core::mem::align_of::<usize>());
    let header = core::mem::size_of::<usize>();
    let Some(size) = layout
        .size()
        .checked_add(alignment - 1 + header)
        .and_then(|size| u32::try_from(size).ok())
    else {
        return core::ptr::null_mut();
    };
    let Ok(base) = backend(size) else {
        return core::ptr::null_mut();
    };
    let address = (base + header + alignment - 1) & !(alignment - 1);
    unsafe { ((address - header) as *mut usize).write(base) };
    address as *mut u8
}

/// # Safety
/// The address must be a live pointer returned by allocate.
pub unsafe fn allocation_base(address: *mut u8) -> usize {
    unsafe { address.cast::<usize>().sub(1).read() }
}
