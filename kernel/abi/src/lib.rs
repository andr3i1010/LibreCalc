// SPDX-License-Identifier: GPL-3.0-only

#![no_std]

mod status;
pub use status::Error;

#[macro_use]
mod macros;
mod functions;
crate::syscall_definitions!(syscalls);

fn text(number: u32, out: &mut [u8]) -> Result<&str, Error> {
    let len = call(number, [out.as_mut_ptr() as usize, out.len(), 0])? as usize;
    let bytes = out.get(..len).ok_or(Error::EABI)?;
    core::str::from_utf8(bytes).map_err(|_| Error::EABI)
}

#[cfg(target_arch = "arm")]
fn call(number: u32, args: [usize; 3]) -> Result<u32, Error> {
    let reply: i32;
    unsafe {
        core::arch::asm!(
            "svc 0",
            inlateout("r0") number => reply,
            in("r1") args[0],
            in("r2") args[1],
            in("r3") args[2],
        );
    }
    if reply < 0 {
        Err(Error::from_code(reply))
    } else {
        Ok(reply as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn error_codes_round_trip() {
        assert_eq!(Error::ENOENT.code(), -5);
        assert_eq!(Error::from_code(-5), Error::ENOENT);
        assert_eq!(Error::from_code(-99).code(), -99);
    }
}
