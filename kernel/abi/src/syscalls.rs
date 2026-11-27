// SPDX-License-Identifier: GPL-3.0-only
use crate::Error;

#[allow(clippy::crate_in_macro_def)]
#[macro_export]
macro_rules! syscall_definitions {
    ($generate:ident $(, $owner:expr, $exit:expr, $allowed:expr, $number:expr, $registers:expr)?) => {
        $crate::$generate! {
            (owner, next $(, $owner, $exit, $allowed, $number, $registers)?);
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
                *next = Some(crate::process::exit(owner, Ok(code)));
                Ok(())
            }
            0x11 => fn launch(app: u32) -> u32 {
                let app = crate::process::app_at(app).ok_or(Error::EINVAL)?;
                *next = Some(crate::process::launch(owner, app)?);
                Ok(0)
            }
            0x12 => fn apps(headers: &mut [u32]) -> u32 {
                Ok(crate::process::apps(headers))
            }
            0x14 => fn pause() -> () {
                *next = Some(crate::process::pause(owner)?);
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
                let [x, y, width, height] = rect.on_screen().ok_or(Error::EINVAL)?;
                if pixels.len() != usize::from(width) * usize::from(height) {
                    return Err(Error::EINVAL);
                }
                n0120_hardware::push_rect(x, y, width, height, pixels.iter().copied());
                Ok(())
            }
            0x21 => fn display_fill_rect(rect: Rect, color: u32) -> () {
                let [x, y, width, height] = rect.on_screen().ok_or(Error::EINVAL)?;
                n0120_hardware::fill_rect(x, y, width, height, color as u16);
                Ok(())
            }
            0x22 => fn display_wait_for_vblank() -> () {
                n0120_hardware::wait_for_vblank().then_some(()).ok_or(Error::ETIMEDOUT)
            }
            0x30 => fn keyboard_scan() -> u64 {
                Ok(crate::keyboard::scan(owner, next))
            }
            0x40 => fn uptime() -> u64 {
                Ok(n0120_hardware::micros())
            }
            0x31 => fn wait_event(keys: u64, timeout_us: u32) -> u64 {
                let end = n0120_hardware::micros() + u64::from(timeout_us);
                loop {
                    let now = crate::keyboard::scan(owner, next);
                    if now != keys || next.is_some() || n0120_hardware::micros() >= end {
                        return Ok(now);
                    }
                    n0120_hardware::sleep_until(end.min(n0120_hardware::micros() + 20_000));
                }
            }
            0x32 => fn modifiers() -> u32 {
                Ok(crate::keyboard::modifiers())
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
            0x60 => fn battery() -> u32 {
                let percent = n0120_hardware::battery_percentage().map_err(|_| Error::EIO)?;
                Ok(u32::from(percent) | u32::from(n0120_hardware::battery_is_charging()) << 8)
            }
        }
    };
}

macro_rules! keys {
    ($($name:ident = $bit:literal $(=> $typed:expr)?,)*) => {
        #[repr(u8)]
        #[derive(Copy, Clone)]
        pub enum Key {
            $($name = $bit,)*
        }
        pub const CHARACTERS: [[u8; 3]; 64] = {
            let mut table = [[0; 3]; 64];
            $($(table[$bit] = $typed;)?)*
            table
        };
    };
}

keys! {
    Left = 0,
    Up = 1,
    Down = 2,
    Right = 3,
    OK = 4,
    Back = 5,
    Home = 6,
    OnOff = 8,
    Shift = 12,
    Alpha = 13,
    XNT = 14 => [b'x', 0, b':'],
    Var = 15 => [0, 0, b';'],
    Toolbox = 16 => [0, 0, b'"'],
    Backspace = 17 => [0, 0, b'%'],
    Exp = 18 => *b"e[a",
    Ln = 19 => [0, b']', b'b'],
    Log = 20 => [0, b'{', b'c'],
    Imaginary = 21 => *b"i}d",
    Comma = 22 => *b",_e",
    Power = 23 => [b'^', 0, b'f'],
    Sin = 24 => [0, 0, b'g'],
    Cos = 25 => [0, 0, b'h'],
    Tan = 26 => [0, 0, b'i'],
    Pi = 27 => [0, b'=', b'j'],
    Sqrt = 28 => [0, b'<', b'k'],
    Square = 29 => [0, b'>', b'l'],
    Seven = 30 => [b'7', 0, b'm'],
    Eight = 31 => [b'8', 0, b'n'],
    Nine = 32 => [b'9', 0, b'o'],
    LeftParenthesis = 33 => [b'(', 0, b'p'],
    RightParenthesis = 34 => [b')', 0, b'q'],
    Four = 36 => [b'4', 0, b'r'],
    Five = 37 => [b'5', 0, b's'],
    Six = 38 => [b'6', 0, b't'],
    Multiplication = 39 => [b'*', 0, b'u'],
    Division = 40 => [b'/', 0, b'v'],
    One = 42 => [b'1', 0, b'w'],
    Two = 43 => [b'2', 0, b'x'],
    Three = 44 => [b'3', 0, b'y'],
    Plus = 45 => [b'+', 0, b'z'],
    Minus = 46 => [b'-', 0, b' '],
    Zero = 48 => [b'0', 0, b'?'],
    Dot = 49 => [b'.', 0, b'!'],
    EE = 50 => [b'E', 0, 0],
    Ans = 51,
    EXE = 52,
}

impl Key {
    pub const fn is_pressed(self, state: u64) -> bool {
        state & (1u64 << self as u8) != 0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i16,
    pub y: i16,
    pub width: i16,
    pub height: i16,
}

impl Rect {
    /// x, y, width and height, when all of it is on the 320 × 240 screen.
    pub fn on_screen(self) -> Option<[u16; 4]> {
        let fits = |start: i16, length: i16, screen: i16| {
            start >= 0 && length >= 0 && i32::from(start) + i32::from(length) <= i32::from(screen)
        };
        (fits(self.x, self.width, 320) && fits(self.y, self.height, 240))
            .then_some([self.x, self.y, self.width, self.height].map(|value| value as u16))
    }
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

impl Arg for u64 {
    const REGISTERS: usize = 2;

    fn encode(self, registers: &mut [u32]) {
        (registers[0], registers[1]) = (self as u32, (self >> 32) as u32);
    }

    unsafe fn decode(
        registers: &[u32],
        _: impl FnOnce(u32, u32, bool) -> Result<(), Error>,
    ) -> Result<Self, Error> {
        Ok(u64::from(registers[0]) | u64::from(registers[1]) << 32)
    }
}

impl Arg for Rect {
    const REGISTERS: usize = 2;

    fn encode(self, registers: &mut [u32]) {
        registers[0] = u32::from(self.x as u16) | u32::from(self.y as u16) << 16;
        registers[1] = u32::from(self.width as u16) | u32::from(self.height as u16) << 16;
    }

    unsafe fn decode(
        registers: &[u32],
        _: impl FnOnce(u32, u32, bool) -> Result<(), Error>,
    ) -> Result<Self, Error> {
        Ok(Rect {
            x: registers[0] as i16,
            y: (registers[0] >> 16) as i16,
            width: registers[1] as i16,
            height: (registers[1] >> 16) as i16,
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
