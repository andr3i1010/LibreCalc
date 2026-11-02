// SPDX-License-Identifier: GPL-3.0-only

use cortex_m::register::control;
use cortex_m_rt::ExceptionFrame;
use librecalc_abi::Error;

librecalc_abi::syscall_definitions!(handlers);

#[unsafe(naked)]
#[unsafe(export_name = "SVCall")]
unsafe extern "C" fn svc_entry() {
    core::arch::naked_asm!(
        "tst lr, #4",
        "ite eq",
        "mrseq r0, MSP",
        "mrsne r0, PSP",
        "b {dispatch}",
        dispatch = sym dispatch,
    );
}

extern "C" fn dispatch(frame: &mut ExceptionFrame) {
    let result = if control::read().npriv().is_privileged() {
        // Only kernel code gets here, and its client wrappers pass a real slice.
        let output =
            unsafe { core::slice::from_raw_parts_mut(frame.r2() as *mut u8, frame.r3() as usize) };
        route(frame.r0(), frame.r1(), output)
    } else {
        // shortcut: deny userspace until scheduling, pointer checks and fault containment exist.
        Err(Error::EACCES)
    };
    let (status, value) = match result {
        Ok(value) => (0, value),
        Err(error) => (error.code(), 0),
    };
    unsafe {
        frame.set_r0(status as u32);
        frame.set_r1(value);
    }
}
