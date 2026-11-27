// SPDX-License-Identifier: GPL-3.0-only

use cortex_m::register::control::{self, Npriv};
use cortex_m_rt::ExceptionFrame;
use librecalc_abi::{Error, Rect};
use n0120_hardware::{EXTERNAL_FLASH_END, EXTERNAL_FLASH_START};

use crate::{memory, process};

#[unsafe(naked)]
#[unsafe(export_name = "SVCall")]
unsafe extern "C" fn svc_entry() {
    core::arch::naked_asm!(
        "tst lr, #4",
        "bne 1f",
        "mrs r0, MSP",
        "mov r1, lr",
        "bl {kernel_call}",
        "bx r0",
        "1:",
        "mrs r0, PSP",
        "tst lr, #0x10",
        "it eq",
        "vstmdbeq r0!, {{s16-s31}}",
        "stmdb r0!, {{r4-r11, lr}}",
        "msr PSP, r0",
        "bl {app_call}",
        "ldmia r0!, {{r4-r11, lr}}",
        "tst lr, #0x10",
        "it eq",
        "vldmiaeq r0!, {{s16-s31}}",
        "msr PSP, r0",
        "bx lr",
        kernel_call = sym kernel_call,
        app_call = sym app_call,
    );
}

extern "C" fn kernel_call(frame: &mut ExceptionFrame, exc_return: u32) -> u32 {
    let Some(stack_ptr) = call(frame, 0) else {
        return exc_return;
    };
    unsafe {
        cortex_m::register::psp::write(stack_ptr as u32 + 36);
        let mut control = control::read();
        control.set_npriv(Npriv::Unprivileged);
        control::write(control);
    }
    0xFFFF_FFFD
}

extern "C" fn app_call(stack_ptr: usize) -> usize {
    let frame = unsafe { &mut *process::frame(stack_ptr).cast::<ExceptionFrame>() };
    let (owner, _) = process::current().expect("app SVC without a process");
    call(frame, owner).unwrap_or(stack_ptr)
}

fn call(frame: &mut ExceptionFrame, owner: u32) -> Option<usize> {
    let mut next = None;
    let result = librecalc_abi::syscall_definitions!(
        kernel_dispatch,
        owner,
        &mut next,
        |address, length, write| {
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
    if next.is_some() {
        return next;
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
    None
}

fn copy<'a>(text: &str, buffer: &'a mut [u8]) -> Result<&'a str, Error> {
    let target = buffer.get_mut(..text.len()).ok_or(Error::EMSGSIZE)?;
    target.copy_from_slice(text.as_bytes());
    core::str::from_utf8(target).map_err(|_| Error::EABI)
}
