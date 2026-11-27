// SPDX-License-Identifier: GPL-3.0-only
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    ptr::NonNull,
};

pub const APP_MAGIC: u32 = u32::from_le_bytes(*b"LCAP");
pub struct AppAllocator(UnsafeCell<rlsf::Tlsf<'static, u16, u8, 15, 8>>);

unsafe impl Sync for AppAllocator {}

impl AppAllocator {
    #[allow(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self(UnsafeCell::new(rlsf::Tlsf::new()))
    }
}

unsafe impl GlobalAlloc for AppAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let heap = unsafe { &mut *self.0.get() };
        if let Some(pointer) = heap.allocate(layout) {
            return pointer.as_ptr();
        }
        let size = (layout.size() * 9 / 8 + layout.align() + 64).next_multiple_of(4096);
        let Ok(address) = crate::alloc(size as u32) else {
            return core::ptr::null_mut();
        };
        let pages = core::ptr::slice_from_raw_parts_mut(address as *mut u8, size);
        unsafe { heap.insert_free_block_ptr(NonNull::new_unchecked(pages)) };
        heap.allocate(layout)
            .map_or(core::ptr::null_mut(), NonNull::as_ptr)
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { (*self.0.get()).deallocate(NonNull::new_unchecked(pointer), layout.align()) }
    }
}

#[repr(C)]
pub struct AppHeader {
    pub magic: u32,
    pub schema: u32,
    pub size: u32,
    pub entry: u32,
    pub name: u32,
    pub icon: u32,
    pub icon_size: u32,
    pub ram_size: u32,
    pub magic_end: u32,
}

impl AppHeader {
    pub fn address(&self) -> u32 {
        self as *const Self as u32
    }

    pub fn icon(&self) -> u32 {
        if self.icon_size != 0 || self.icon.saturating_add(4) > self.size {
            return 0;
        }
        unsafe { ((self.address() + self.icon) as *const u32).read_unaligned() }
    }

    pub fn name(&self) -> &str {
        let start = (self.address() + self.name) as *const u8;
        let bytes = unsafe { core::slice::from_raw_parts(start, (self.size - self.name) as usize) };
        let length = bytes
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(bytes.len());
        core::str::from_utf8(&bytes[..length]).unwrap_or("?")
    }
}

#[macro_export]
macro_rules! app {
    ($name:literal, $main:path, $icon:expr) => {
        #[allow(non_upper_case_globals)]
        #[unsafe(no_mangle)]
        #[unsafe(link_section = ".rodata.app_name")]
        static __app_name: [u8; $name.len() + 1] =
            *concat!($name, "\0").as_bytes().first_chunk().unwrap();

        #[allow(non_upper_case_globals)]
        #[unsafe(no_mangle)]
        #[unsafe(link_section = ".rodata.app_icon")]
        static __app_icon: u32 = $icon;

        #[unsafe(no_mangle)]
        extern "C" fn _start() -> ! {
            $crate::app::start($main)
        }

        #[panic_handler]
        fn panic(_: &core::panic::PanicInfo) -> ! {
            $crate::app::crash()
        }

        #[global_allocator]
        static ALLOCATOR: $crate::AppAllocator = $crate::AppAllocator::new();
    };
}

pub fn start(main: impl FnOnce() -> u32) -> ! {
    unsafe extern "C" {
        static __data_load: u8;
        static mut __data_start: u8;
        static __data_end: u8;
    }
    unsafe {
        let length = &raw const __data_end as usize - &raw const __data_start as usize;
        core::ptr::copy_nonoverlapping(&raw const __data_load, &raw mut __data_start, length);
    }
    let _ = crate::exit(main());
    crash()
}

#[doc(hidden)]
pub fn crash() -> ! {
    unsafe { core::arch::asm!("udf #0", options(noreturn)) }
}
