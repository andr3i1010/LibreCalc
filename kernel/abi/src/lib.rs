// SPDX-License-Identifier: GPL-3.0-only

#![no_std]

extern crate alloc;

mod status;
pub use status::Error;

pub mod heap;
#[doc(hidden)]
pub mod transport;
pub use alloc::string::String;
pub use heap::AppAllocator;

mod functions;
mod macros;
crate::syscall_definitions!(clients);
