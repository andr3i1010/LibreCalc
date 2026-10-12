// SPDX-License-Identifier: GPL-3.0-only

macro_rules! syscalls {
    (@client) => {};
    (@client
        #[id($id:literal)]
        pub fn $name:ident() -> &'static str $body:block
        $($rest:tt)*
    ) => {
        pub fn $name(out: &mut [u8]) -> Result<&str, Error> {
            text($id, out)
        }
        syscalls!(@client $($rest)*);
    };
    (@client
        #[id($id:literal)]
        pub fn $name:ident() -> ! $body:block
        $($rest:tt)*
    ) => {
        pub fn $name() -> Result<(), Error> {
            call($id, [0; 3]).map(|_| ())
        }
        syscalls!(@client $($rest)*);
    };
    (@ids $(
        #[id($id:literal)]
        pub fn $name:ident() -> $result:ty $body:block
    )*) => {
        #[allow(non_upper_case_globals)]
        pub mod id {
            $(pub const $name: u32 = $id;)*
        }

        const _: () = {
            #[deny(unreachable_patterns)]
            match 0_u32 {
                $($id => (),)*
                _ => (),
            }
        };
    };
    (@$mode:ident $($unsupported:tt)*) => {
        compile_error!("supported syscalls take no arguments and return &'static str or !");
    };
    ($($definitions:tt)*) => {
        syscalls!(@ids $($definitions)*);
        syscalls!(@client $($definitions)*);
    };
}
