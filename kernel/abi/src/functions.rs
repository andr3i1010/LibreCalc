// SPDX-License-Identifier: GPL-3.0-only

#[macro_export]
macro_rules! syscall_definitions {
    ($generate:ident) => {
        $generate! {
            #[id(0)]
            pub fn os_version() -> &'static str {
                env!("CARGO_PKG_VERSION")
            }

            #[id(1)]
            pub fn abi_version() -> &'static str {
                "1.0.0"
            }

            #[id(2)]
            pub fn reset() -> ! {
                n0120_hardware::reset()
            }

            #[id(3)]
            pub fn power_off() -> ! {
                n0120_hardware::power_off()
            }
        }
    };
}
