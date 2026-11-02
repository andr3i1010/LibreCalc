// SPDX-License-Identifier: GPL-3.0-only

// `$(@$u:tt)?` never matches anything; it only gives the optional `unsafe` a variable to repeat on.
#[macro_export]
macro_rules! clients {
    ($(
        #[id($id:literal)]
        $(#[$attr:meta])*
        pub $(unsafe $(@$u:tt)?)? fn $name:ident($($arg:ident: u32)?) -> Result<$ok:ty, $err:ty> $body:block
    )*) => {
        $(
            $(#[$attr])*
            pub $(unsafe $($u)?)? fn $name($($arg: u32)?) -> Result<$ok, $err> {
                <$ok as $crate::transport::Reply>::receive($id, 0 $(+ $arg)?)
            }
        )*

        // A repeated ID must not compile. Only checked here: lints stay silent in other crates' macros.
        const _: () = {
            #[deny(unreachable_patterns)]
            match 0_u32 {
                $($id => (),)*
                _ => (),
            }
        };
    };
}

#[macro_export]
macro_rules! handlers {
    ($(
        #[id($id:literal)]
        $(#[$attr:meta])*
        pub $(unsafe $(@$u:tt)?)? fn $name:ident($($arg:ident: u32)?) -> Result<$ok:ty, $err:ty> $body:block
    )*) => {
        $(
            $(#[$attr])*
            pub $(unsafe $($u)?)? fn $name($($arg: u32)?) -> Result<$ok, $err> $body
        )*

        #[allow(unused_unsafe)]
        pub fn route(number: u32, argument: u32, output: &mut [u8]) -> Result<u32, $crate::Error> {
            match number {
                $($id => {
                    $(let $arg = argument;)?
                    $crate::transport::Reply::send(unsafe { $name($($arg)?) }?, output)
                })*
                _ => Err($crate::Error::ENOSYS),
            }
        }
    };
}
