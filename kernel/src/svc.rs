// SPDX-License-Identifier: GPL-3.0-only

#[cfg(target_arch = "arm")]
use core::arch::naked_asm;
#[cfg(target_arch = "arm")]
use cortex_m_rt::ExceptionFrame;
use librecalc_abi::Error;

macro_rules! syscalls {
    (@route $number:ident, $args:ident, $privileged:ident;) => {
        Error::ENOSYS.code()
    };
    (@route $number:ident, $args:ident, $privileged:ident;
        #[id($id:literal)]
        pub fn $name:ident() -> &'static str $body:block
        $($rest:tt)*
    ) => {
        if $number == $id {
            if !$privileged {
                Error::EACCES.code()
            } else {
                write_text($name().as_bytes(), $args)
            }
        } else {
            syscalls!(@route $number, $args, $privileged; $($rest)*)
        }
    };
    (@route $number:ident, $args:ident, $privileged:ident;
        #[id($id:literal)]
        pub fn $name:ident() -> ! $body:block
        $($rest:tt)*
    ) => {
        if $number == $id {
            if !$privileged {
                Error::EACCES.code()
            } else {
                $name()
            }
        } else {
            syscalls!(@route $number, $args, $privileged; $($rest)*)
        }
    };
    (@handlers $(
        #[id($id:literal)]
        pub fn $name:ident() -> $result:ty $body:block
    )*) => {
        $(pub fn $name() -> $result $body)*
    };
    (@$mode:ident $($unsupported:tt)*) => {
        compile_error!("supported syscalls take no arguments and return &'static str or !");
    };
    ($($definitions:tt)*) => {
        syscalls!(@handlers $($definitions)*);

        pub(super) fn route(number: u32, args: [usize; 3], privileged: bool) -> i32 {
            syscalls!(@route number, args, privileged; $($definitions)*)
        }
    };
}
librecalc_abi::syscall_definitions!(syscalls);

fn write_text(text: &[u8], [address, capacity, _]: [usize; 3]) -> i32 {
    if capacity < text.len() {
        return Error::EMSGSIZE.code();
    }
    if address == 0 || address.checked_add(text.len()).is_none() {
        return Error::EFAULT.code();
    }
    unsafe { core::ptr::copy_nonoverlapping(text.as_ptr(), address as *mut u8, text.len()) };
    text.len() as i32
}

#[cfg(target_arch = "arm")]
#[unsafe(naked)]
#[unsafe(export_name = "SVCall")]
unsafe extern "C" fn svc_entry() {
    naked_asm!(
        "tst lr, #4",
        "ite eq",
        "mrseq r0, MSP",
        "mrsne r0, PSP",
        "tst lr, #16",
        "bne 2f",
        "add r0, r0, #72",
        "2:",
        "b {dispatch}",
        dispatch = sym dispatch,
    );
}

#[cfg(target_arch = "arm")]
extern "C" fn dispatch(frame: &mut ExceptionFrame) {
    let privileged = cortex_m::register::control::read().npriv().is_privileged();
    let reply = route(
        frame.r0(),
        [
            frame.r1() as usize,
            frame.r2() as usize,
            frame.r3() as usize,
        ],
        privileged,
    );
    unsafe { frame.set_r0(reply as u32) };
}
