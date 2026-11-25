// SPDX-License-Identifier: GPL-3.0-only

use cortex_m_rt::ExceptionFrame;
use librecalc_abi::{Error, Rect};
use n0120_hardware::{EXTERNAL_FLASH_END, EXTERNAL_FLASH_START};

use crate::{memory, process};

#[unsafe(naked)]
#[unsafe(export_name = "SVCall")]
unsafe extern "C" fn svc_entry() {
    core::arch::naked_asm!(
        "tst lr, #4",
        "ite eq",
        "mrseq r0, MSP",
        "mrsne r0, PSP",
        "mov r1, lr",
        "bl {dispatch}",
        "bx r0",
        dispatch = sym dispatch,
    );
}

extern "C" fn dispatch(frame: &mut ExceptionFrame, exc_return: u32) -> u32 {
    let owner = if exc_return & 4 != 0 {
        process::current_id().expect("app SVC without a process")
    } else {
        0
    };
    let mut exit = None;
    let result = librecalc_abi::syscall_definitions!(
        kernel_dispatch,
        owner,
        &mut exit,
        |address, length, write| {
            // Apps may also hand the kernel data to read straight from flash, e.g. images.
            let in_flash = address >= EXTERNAL_FLASH_START
                && address
                    .checked_add(length)
                    .is_some_and(|end| end <= EXTERNAL_FLASH_END);
            if owner == 0
                || memory::owns(owner, address as usize, length as usize)
                || !write && in_flash
            {
                Ok(())
            } else {
                Err(Error::EFAULT)
            }
        },
        frame.r12(),
        [frame.r0(), frame.r1(), frame.r2(), frame.r3()]
    );
    if let Some(code) = exit {
        return process::exit(code, owner != 0);
    }
    let (status, value) = match result {
        Ok(value) => (0, value),
        Err(error) => (error.code(), 0),
    };
    unsafe {
        frame.set_r0(status as u32);
        frame.set_r1(value as u32);
        frame.set_r2((value >> 32) as u32);
    }
    exc_return
}

fn copy<'a>(text: &str, buffer: &'a mut [u8]) -> Result<&'a str, Error> {
    let target = buffer.get_mut(..text.len()).ok_or(Error::EMSGSIZE)?;
    target.copy_from_slice(text.as_bytes());
    core::str::from_utf8(target).map_err(|_| Error::EABI)
}
