// SPDX-License-Identifier: GPL-3.0-only

use crate::Error;
use alloc::string::String;

pub fn copy_str(value: &str) -> Result<String, Error> {
    let mut text = String::new();
    text.try_reserve_exact(value.len())
        .map_err(|_| Error::ENOMEM)?;
    text.push_str(value);
    Ok(text)
}

/// A syscall result: how the kernel sends it and how the client receives it.
pub trait Reply: Sized {
    fn receive(number: u32, argument: u32) -> Result<Self, Error>;
    fn send(self, output: &mut [u8]) -> Result<u32, Error>;
}

impl Reply for () {
    fn receive(number: u32, argument: u32) -> Result<Self, Error> {
        call(number, argument, &mut []).map(|_| ())
    }

    fn send(self, _: &mut [u8]) -> Result<u32, Error> {
        Ok(0)
    }
}

impl Reply for u32 {
    fn receive(number: u32, argument: u32) -> Result<Self, Error> {
        call(number, argument, &mut [])
    }

    fn send(self, _: &mut [u8]) -> Result<u32, Error> {
        Ok(self)
    }
}

impl Reply for String {
    fn receive(number: u32, argument: u32) -> Result<Self, Error> {
        // shortcut: version replies fit in 64 bytes; revisit when adding larger text services.
        let mut output = [0; 64];
        let length = call(number, argument, &mut output)? as usize;
        let bytes = output.get(..length).ok_or(Error::EABI)?;
        copy_str(core::str::from_utf8(bytes).map_err(|_| Error::EABI)?)
    }

    fn send(self, output: &mut [u8]) -> Result<u32, Error> {
        output
            .get_mut(..self.len())
            .ok_or(Error::EMSGSIZE)?
            .copy_from_slice(self.as_bytes());
        Ok(self.len() as u32)
    }
}

#[cfg(all(target_arch = "arm", not(test)))]
pub fn call(number: u32, argument: u32, output: &mut [u8]) -> Result<u32, Error> {
    let status: i32;
    let value: u32;
    unsafe {
        core::arch::asm!(
            "svc 0",
            inlateout("r0") number => status,
            inlateout("r1") argument => value,
            in("r2") output.as_mut_ptr(),
            in("r3") output.len(),
        );
    }
    match status {
        0 => Ok(value),
        error => Err(Error::from_code(error)),
    }
}

#[cfg(all(not(target_arch = "arm"), not(test)))]
pub fn call(_: u32, _: u32, _: &mut [u8]) -> Result<u32, Error> {
    Err(Error::ENOSYS)
}

#[cfg(test)]
pub fn call(number: u32, argument: u32, output: &mut [u8]) -> Result<u32, Error> {
    crate::tests::kernel::route(number, argument, output)
}
