// SPDX-License-Identifier: GPL-3.0-only
use core::{cell::RefCell, fmt::Write, ops::Range};
use cortex_m::{
    interrupt::Mutex,
    peripheral::{FPU, SCB},
    register::control::{self, Npriv},
};
use cortex_m_rt::ExceptionFrame;
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point, RgbColor, Size},
    primitives::Rectangle,
};
use librecalc_abi::{AppHeader, Error};
use n0120_hardware::{Console, Display};

use crate::memory;

const APP_FLASH: Range<u32> = 0x9010_0000..0x9040_0000;
const STACK_SIZE: usize = 32 * 1024;
const MAX_PROCESSES: usize = 2;

#[derive(Clone, Copy)]
pub struct Process {
    pub id: u32,
    pub stack_start: usize,
    pub stack_ptr: usize,
    pub app: &'static AppHeader,
}

struct Processes {
    entries: [Option<Process>; MAX_PROCESSES],
    current: Option<u32>,
    next_id: u32,
}

static PROCESSES: Mutex<RefCell<Processes>> = Mutex::new(RefCell::new(Processes {
    entries: [None; MAX_PROCESSES],
    current: None,
    next_id: 1,
}));

pub fn current_id() -> Option<u32> {
    current().map(|process| process.id)
}

fn current() -> Option<Process> {
    cortex_m::interrupt::free(|cs| {
        let processes = PROCESSES.borrow(cs).borrow();
        processes
            .entries
            .iter()
            .flatten()
            .find(|process| Some(process.id) == processes.current)
            .copied()
    })
}

pub fn create(app: &'static AppHeader) -> Result<Process, Error> {
    cortex_m::interrupt::free(|cs| {
        let mut processes = PROCESSES.borrow(cs).borrow_mut();
        let slot = processes
            .entries
            .iter()
            .position(Option::is_none)
            .ok_or(Error::EBUSY)?;
        let id = processes.next_id;
        // ram_size comes from the app's header, so it may be anything.
        let total = STACK_SIZE
            .checked_add(app.ram_size as usize)
            .ok_or(Error::ENOMEM)?;
        let stack_start = memory::alloc(id, total, false)?;
        let stack_ptr = stack_start + STACK_SIZE - 32;
        let frame = stack_ptr as *mut u32;
        unsafe {
            frame.add(6).write((app.address() + app.entry) & !1); // PC
            frame.add(7).write(1 << 24); // xPSR: Thumb
        }
        let process = Process {
            id,
            stack_start,
            stack_ptr,
            app,
        };
        processes.entries[slot] = Some(process);
        processes.next_id += 1;
        Ok(process)
    })
}

unsafe extern "C" {
    #[link_name = "__app_header"]
    static HOME: AppHeader;
}

pub fn init() {
    unsafe {
        (*SCB::PTR).shcsr.modify(|value| value | 0b111 << 16);
        (*FPU::PTR).fpccr.modify(|value| value & !(1 << 30));
    }
}

pub fn start_home() -> u32 {
    let address = (&raw const HOME) as u32;
    if !APP_FLASH.contains(&address)
        || APP_FLASH.end - address < core::mem::size_of::<AppHeader>() as u32
    {
        panic!("no home app at {address:#010x}");
    }
    let home = unsafe { &HOME };
    if !home.is_valid() || home.size > APP_FLASH.end - address {
        panic!("invalid home app at {address:#010x}");
    }
    let process = create(home).expect("app RAM unavailable");
    assert_eq!(
        process.stack_start,
        memory::APP_RAM[0].start,
        "home RAM was not empty"
    );
    memory::map_pages(process.id);
    cortex_m::interrupt::free(|cs| PROCESSES.borrow(cs).borrow_mut().current = Some(process.id));
    unsafe {
        cortex_m::register::psp::write(process.stack_ptr as u32);
        let mut value = control::read();
        value.set_npriv(Npriv::Unprivileged);
        control::write(value);
    }
    cortex_m::asm::isb();
    0xFFFF_FFFD
}

pub fn exit(code: u32, from_app: bool) -> u32 {
    if from_app {
        let process = current().expect("app exit without a process");
        let mut display = Display;
        let _ = display.clear(Rgb565::BLACK);
        let mut console = Console::new(
            &mut display,
            Rectangle::new(Point::zero(), Size::new(320, 240)),
            Rgb565::WHITE,
            Rgb565::BLACK,
        );
        let _ = writeln!(
            console,
            "PID {} ({}) exited with code {code}.",
            process.id,
            process.app.name()
        );
        let _ = writeln!(console, "Press any key to restart home.");
        while n0120_hardware::read_keyboard() != 0 {}
        while n0120_hardware::read_keyboard() == 0 {}
        while n0120_hardware::read_keyboard() != 0 {}
        memory::free_by_owner(process.id);
        cortex_m::interrupt::free(|cs| {
            let mut processes = PROCESSES.borrow(cs).borrow_mut();
            processes.current = None;
            for entry in &mut processes.entries {
                if entry.is_some_and(|entry| entry.id == process.id) {
                    *entry = None;
                }
            }
        });
    }
    start_home()
}

#[unsafe(naked)]
#[unsafe(export_name = "MemoryManagement")]
unsafe extern "C" fn fault_entry() {
    core::arch::naked_asm!(
        "and r0, lr, #0xC",
        "cmp r0, #0xC",
        "bne {kernel}",
        "mrs r0, PSP",
        "b {fault}",
        ".global BusFault",
        ".thumb_set BusFault, MemoryManagement",
        ".global UsageFault",
        ".thumb_set UsageFault, MemoryManagement",
        kernel = sym HardFault,
        fault = sym fault,
    );
}

unsafe extern "C" {
    fn HardFault();
}

extern "C" fn fault(frame: *const ExceptionFrame) -> ! {
    let scb = unsafe { &*SCB::PTR };
    let status = scb.cfsr.read();
    let frame = (status & (1 << 4 | 1 << 12) == 0).then(|| unsafe { &*frame });
    let pc = frame.map_or(0, ExceptionFrame::pc);
    let process = current().expect("app fault without a process");
    panic!(
        "PID {} ({}) crashed: PC {pc:#010x}, CFSR {status:#010x}",
        process.id,
        process.app.name()
    );
}
