// SPDX-License-Identifier: GPL-3.0-only

#![no_std]
mod status;
pub use status::Error;

pub mod app;
pub use app::{AppAllocator, AppHeader};
pub mod syscalls;
pub use syscalls::Rect;

pub mod setting {
    pub const THEME: u32 = 0;
    pub const COUNT: usize = 1;
}
crate::syscall_definitions!(app_calls);

fn call(number: u32, [a0, a1, a2, a3]: [u32; 4]) -> Result<u64, Error> {
    let (status, low, high): (i32, u32, u32);
    unsafe {
        core::arch::asm!(
            "svc 0",
            in("r12") number,
            inlateout("r0") a0 => status,
            inlateout("r1") a1 => low,
            inlateout("r2") a2 => high,
            in("r3") a3,
        );
    }
    match status {
        0 => Ok(u64::from(low) | u64::from(high) << 32),
        error => Err(Error::from_code(error)),
    }
}
