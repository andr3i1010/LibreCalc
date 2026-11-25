// SPDX-License-Identifier: GPL-3.0-only
use crate::Error;

#[allow(clippy::crate_in_macro_def)]
#[macro_export]
macro_rules! syscall_definitions {
    ($generate:ident $(, $owner:expr, $exit:expr, $allowed:expr, $number:expr, $registers:expr)?) => {
        $crate::$generate! {
            (owner, exit $(, $owner, $exit, $allowed, $number, $registers)?);
            0x00 => fn os_version(buffer: &mut [u8]) -> &str {
                copy(env!("CARGO_PKG_VERSION"), buffer)
            }
            0x01 => fn abi_version(buffer: &mut [u8]) -> &str {
                copy("1.0.0", buffer)
            }
            0x02 => fn reset() -> () {
                n0120_hardware::reset()
            }
            0x03 => fn power_off() -> () {
                n0120_hardware::power_off()
            }
            0x10 => fn exit(code: u32) -> () {
                *exit = Some(code);
                Ok(())
            }
            0x1A => fn alloc(size: u32) -> u32 {
                let address = crate::memory::alloc(owner, size as usize, true)?;
                if owner != 0 {
                    crate::memory::map_pages(owner);
                }
                Ok(address as u32)
            }
            0x1B => unsafe fn free(address: u32) -> () {
                crate::memory::free(owner, address as usize)?;
                if owner != 0 {
                    crate::memory::map_pages(owner);
                }
                Ok(())
            }
            0x20 => fn display_push_rect(rect: Rect, pixels: &[u16]) -> () {
                if pixels.len() != usize::from(rect.width) * usize::from(rect.height) {
                    return Err(Error::EINVAL);
                }
                n0120_hardware::push_rect(rect.x, rect.y, rect.width, rect.height, pixels.iter().copied());
                Ok(())
            }
            0x21 => fn display_fill_rect(rect: Rect, color: u32) -> () {
                n0120_hardware::fill_rect(rect.x, rect.y, rect.width, rect.height, color as u16);
                Ok(())
            }
            0x22 => fn display_wait_for_vblank() -> () {
                n0120_hardware::wait_for_vblank().then_some(()).ok_or(Error::ETIMEDOUT)
            }
            0x30 => fn keyboard_scan() -> u64 {
                Ok(n0120_hardware::read_keyboard())
            }
            0x40 => fn uptime() -> u64 {
                Ok(n0120_hardware::micros())
            }
            /// Waits until a key is pressed or released, or `timeout_us` passes; returns the keys held then.
            0x31 => fn wait_event(timeout_us: u32) -> u64 {
                let end = n0120_hardware::micros() + u64::from(timeout_us);
                let keys = n0120_hardware::read_keyboard();
                loop {
                    let now = n0120_hardware::read_keyboard();
                    if now != keys || n0120_hardware::micros() >= end {
                        return Ok(now);
                    }
                    n0120_hardware::sleep_until(end.min(n0120_hardware::micros() + 20_000));
                }
            }
            0x41 => fn sleep_us(microseconds: u32) -> () {
                let end = n0120_hardware::micros() + u64::from(microseconds);
                while n0120_hardware::micros() < end {
                    n0120_hardware::sleep_until(end);
                }
                Ok(())
            }
            0x50 => fn setting(key: u32) -> u32 {
                Ok(crate::settings::get(key))
            }
            0x51 => fn set_setting(key: u32, value: u32) -> () {
                crate::settings::set(key, value)
            }
        }
    };
}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

pub trait Arg: Sized {
    const REGISTERS: usize;

    fn encode(self, registers: &mut [u32]);
    unsafe fn decode(
        registers: &[u32],
        check: impl FnOnce(u32, u32, bool) -> Result<(), Error>,
    ) -> Result<Self, Error>;
}

impl Arg for u32 {
    const REGISTERS: usize = 1;

    fn encode(self, registers: &mut [u32]) {
        registers[0] = self;
    }

    unsafe fn decode(
        registers: &[u32],
        _: impl FnOnce(u32, u32, bool) -> Result<(), Error>,
    ) -> Result<Self, Error> {
        Ok(registers[0])
    }
}

impl Arg for Rect {
    const REGISTERS: usize = 2;

    fn encode(self, registers: &mut [u32]) {
        registers[0] = u32::from(self.x) | u32::from(self.y) << 16;
        registers[1] = u32::from(self.width) | u32::from(self.height) << 16;
    }

    unsafe fn decode(
        registers: &[u32],
        _: impl FnOnce(u32, u32, bool) -> Result<(), Error>,
    ) -> Result<Self, Error> {
        Ok(Rect {
            x: registers[0] as u16,
            y: (registers[0] >> 16) as u16,
            width: registers[1] as u16,
            height: (registers[1] >> 16) as u16,
        })
    }
}

impl<T> Arg for &[T] {
    const REGISTERS: usize = 2;

    fn encode(self, registers: &mut [u32]) {
        registers[0] = self.as_ptr() as u32;
        registers[1] = self.len() as u32;
    }

    unsafe fn decode(
        registers: &[u32],
        check: impl FnOnce(u32, u32, bool) -> Result<(), Error>,
    ) -> Result<Self, Error> {
        let address = unsafe { checked::<T>(registers, check, false)? };
        Ok(unsafe { core::slice::from_raw_parts(address as *const T, registers[1] as usize) })
    }
}

impl<T> Arg for &mut [T] {
    const REGISTERS: usize = 2;

    fn encode(self, registers: &mut [u32]) {
        Arg::encode(&*self, registers);
    }

    unsafe fn decode(
        registers: &[u32],
        check: impl FnOnce(u32, u32, bool) -> Result<(), Error>,
    ) -> Result<Self, Error> {
        let address = unsafe { checked::<T>(registers, check, true)? };
        Ok(unsafe { core::slice::from_raw_parts_mut(address as *mut T, registers[1] as usize) })
    }
}

/// The address of a slice an app passed, once it's aligned and the app may access all of it.
/// An empty slice gets a dangling address, since apps may pass anything for one.
unsafe fn checked<T>(
    registers: &[u32],
    check: impl FnOnce(u32, u32, bool) -> Result<(), Error>,
    write: bool,
) -> Result<u32, Error> {
    let (address, length) = (registers[0], registers[1]);
    let bytes = length
        .checked_mul(size_of::<T>() as u32)
        .ok_or(Error::EFAULT)?;
    if bytes == 0 {
        return Ok(core::ptr::NonNull::<T>::dangling().as_ptr() as u32);
    }
    if !(address as usize).is_multiple_of(align_of::<T>()) {
        return Err(Error::EFAULT);
    }
    check(address, bytes, write)?;
    Ok(address)
}

pub trait Ret: Sized {
    fn encode(self) -> u64;

    /// # Safety
    /// `raw` must be the kernel's encoding for this syscall.
    unsafe fn decode(raw: u64) -> Result<Self, Error>;
}

impl Ret for () {
    fn encode(self) -> u64 {
        0
    }

    unsafe fn decode(_: u64) -> Result<Self, Error> {
        Ok(())
    }
}

impl Ret for u32 {
    fn encode(self) -> u64 {
        self.into()
    }

    unsafe fn decode(raw: u64) -> Result<Self, Error> {
        Ok(raw as u32)
    }
}

impl Ret for u64 {
    fn encode(self) -> u64 {
        self
    }

    unsafe fn decode(raw: u64) -> Result<Self, Error> {
        Ok(raw)
    }
}

impl Ret for &str {
    fn encode(self) -> u64 {
        self.as_ptr() as u64 | (self.len() as u64) << 32
    }

    unsafe fn decode(raw: u64) -> Result<Self, Error> {
        core::str::from_utf8(unsafe {
            core::slice::from_raw_parts(raw as u32 as *const u8, (raw >> 32) as usize)
        })
        .map_err(|_| Error::EABI)
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! app_calls {
    (($owner:ident, $exit:ident);
        $(
            $(#[$attr:meta])*
            $id:literal => $(unsafe $(@$u:tt)?)? fn $name:ident ($($arg:ident: $ty:ty),*) -> $ret:ty $body:block
        )*
    ) => {
        $(
            $(#[$attr])*
            #[allow(unused_mut, unused_assignments, unused_variables)]
            pub $(unsafe $($u)?)? fn $name($($arg: $ty),*) -> Result<$ret, Error> {
                let (mut registers, mut next) = ([0; 4], 0);
                $(
                    $crate::syscalls::Arg::encode($arg, &mut registers[next..]);
                    next += <$ty as $crate::syscalls::Arg>::REGISTERS;
                )*
                unsafe { <$ret as $crate::syscalls::Ret>::decode(call($id, registers)?) }
            }
        )*
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! kernel_dispatch {
    (($owner:ident, $exit:ident, $owner_value:expr, $exit_value:expr, $allowed:expr, $number:expr, $registers:expr);
        $(
            $(#[$attr:meta])*
            $id:literal => $(unsafe $(@$u:tt)?)? fn $name:ident ($($arg:ident: $ty:ty),*) -> $ret:ty $body:block
        )*
    ) => {{
        let $owner = $owner_value;
        let $exit = $exit_value;
        let registers: [u32; 4] = $registers;
        #[deny(unreachable_patterns)]
        match $number {
            #[allow(unused_mut, unused_assignments, unused_variables)]
            $($id => (|| -> Result<$ret, $crate::Error> {
                let mut next = 0;
                $(
                    let $arg = unsafe {
                        <$ty as $crate::syscalls::Arg>::decode(&registers[next..], $allowed)?
                    };
                    next += <$ty as $crate::syscalls::Arg>::REGISTERS;
                )*
                $body
            })().map($crate::syscalls::Ret::encode),)*
            _ => Err($crate::Error::ENOSYS),
        }
    }};
}
