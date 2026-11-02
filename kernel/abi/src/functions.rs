// SPDX-License-Identifier: GPL-3.0-only

#[macro_export]
// Bodies expand in the kernel, where its private modules are available.
#[allow(clippy::crate_in_macro_def)]
macro_rules! syscall_definitions {
    ($generate:ident) => {
        $crate::$generate! {
            #[id(0)]
            pub fn os_version() -> Result<$crate::String, $crate::Error> {
                $crate::transport::copy_str(env!("CARGO_PKG_VERSION"))
            }

            #[id(1)]
            pub fn abi_version() -> Result<$crate::String, $crate::Error> {
                $crate::transport::copy_str("1.0.0")
            }

            #[id(2)]
            pub fn reset() -> Result<(), $crate::Error> {
                n0120_hardware::reset()
            }

            #[id(3)]
            pub fn power_off() -> Result<(), $crate::Error> {
                n0120_hardware::power_off()
            }

            #[id(0x1A)]
            pub fn memory_alloc(size: u32) -> Result<u32, $crate::Error> {
                crate::memory::alloc_heap(crate::process::current_id().unwrap_or(0), size as usize)
                    .map(|address| address as u32)
            }

            #[id(0x1B)]
            /// # Safety
            /// The allocation must no longer be used, including by an active syscall.
            pub unsafe fn memory_free(address: u32) -> Result<(), $crate::Error> {
                crate::memory::free(crate::process::current_id().unwrap_or(0), address as usize)
            }
        }
    };
}
