// SPDX-License-Identifier: GPL-3.0-only

#![no_std]

pub use graphite_ui::*;

struct NoAllocator;

unsafe impl core::alloc::GlobalAlloc for NoAllocator {
    unsafe fn alloc(&self, _: core::alloc::Layout) -> *mut u8 {
        core::ptr::null_mut()
    }

    unsafe fn dealloc(&self, _: *mut u8, _: core::alloc::Layout) {}
}

#[global_allocator]
static ALLOCATOR: NoAllocator = NoAllocator;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    librecalc_abi::app::crash()
}
