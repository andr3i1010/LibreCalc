// SPDX-License-Identifier: GPL-3.0-only
use core::cell::RefCell;
use cortex_m::{
    interrupt::Mutex,
    peripheral::{FPU, SCB},
};
use librecalc_abi::{APP_FLASH, AppHeader, Error, SCREEN_COPY, app::APP_MAGIC};

use crate::memory::{self, PAGE_SIZE};

const STACK_SIZE: usize = 32 * 1024;
const MAX_PROCESSES: usize = 8;
const GLOBALS: usize = memory::APP_RAM[0].start;
const AREA: usize = SCREEN_COPY - GLOBALS + 320 * 240 * 2;

#[derive(Clone, Copy, PartialEq)]
pub enum State {
    Running,
    Paused,
}

#[derive(Clone, Copy)]
pub struct Process {
    pub app: &'static AppHeader,
    pub state: State,
    pub stack_ptr: usize,
    pub parent: Option<u32>,
    pub globals: usize,
}

type Processes = [Option<Process>; MAX_PROCESSES];

static PROCESSES: Mutex<RefCell<Processes>> = Mutex::new(RefCell::new([None; MAX_PROCESSES]));

pub fn current() -> Option<(u32, Process)> {
    cortex_m::interrupt::free(|cs| {
        let processes = PROCESSES.borrow(cs).borrow();
        let mut entries = (1..).zip(processes.iter());
        entries.find_map(|(id, entry)| Some((id, entry.filter(|p| p.state == State::Running)?)))
    })
}

unsafe extern "C" {
    #[link_name = "__app_header"]
    static BOOT: AppHeader;
}

pub fn init() {
    unsafe {
        (*SCB::PTR).shcsr.modify(|value| value | 0b111 << 16);
        (*SCB::PTR).shpr[7].write(0xF0);
        (*FPU::PTR).fpccr.modify(|value| value & !(1 << 30));
    }
}
pub fn app_at(address: u32) -> Option<&'static AppHeader> {
    let fits = APP_FLASH.contains(&address)
        && address.is_multiple_of(4096)
        && APP_FLASH.end - address >= core::mem::size_of::<AppHeader>() as u32;
    let app = fits.then(|| unsafe { &*(address as *const AppHeader) })?;
    let valid = app.magic == APP_MAGIC
        && app.magic_end == APP_MAGIC
        && app.schema == 1
        && app.size >= core::mem::size_of::<AppHeader>() as u32
        && app.entry < app.size
        && app.name < app.size;
    (valid && app.size <= APP_FLASH.end - address).then_some(app)
}

pub fn apps(headers: &mut [u32]) -> u32 {
    let (mut count, mut next) = (0, app_at(APP_FLASH.start));
    while let Some(app) = next {
        if let Some(header) = headers.get_mut(count) {
            *header = app.address();
        }
        count += 1;
        next = app_at((app.address() + app.size).next_multiple_of(4096));
    }
    count as u32
}

pub fn launch(owner: u32, app: &'static AppHeader) -> Result<usize, Error> {
    cortex_m::interrupt::free(|cs| {
        let processes = &mut *PROCESSES.borrow(cs).borrow_mut();
        let paused = processes.iter().position(|entry| {
            entry.is_some_and(|p| p.state == State::Paused && core::ptr::eq(p.app, app))
        });
        let globals = memory::alloc(owner, PAGE_SIZE, true)?;
        unsafe {
            core::ptr::copy_nonoverlapping(GLOBALS as *const u8, globals as *mut u8, PAGE_SIZE)
        };
        memory::free(owner, GLOBALS)?;
        let stack_ptr = cortex_m::register::psp::read() as usize;
        let me = processes[owner as usize - 1].as_mut().unwrap();
        *me = Process {
            state: State::Paused,
            stack_ptr,
            globals,
            ..*me
        };

        let (id, next) = match paused {
            Some(slot) => (slot as u32 + 1, Ok(())),
            None => match create(processes, app, Some(owner)) {
                Ok(stack_ptr) => return Ok(stack_ptr),
                Err(error) => (owner, Err(error)),
            },
        };
        let process = processes[id as usize - 1].as_mut().unwrap();
        if id != owner {
            process.parent = Some(owner);
        }
        take_area(id).expect("no room to restore");
        let globals = process.globals as *const u8;
        unsafe { core::ptr::copy_nonoverlapping(globals, GLOBALS as *mut u8, PAGE_SIZE) };
        let _ = memory::free(id, process.globals);
        (process.state, process.globals) = (State::Running, 0);
        memory::map_pages(id);
        // Its launch call (from when it was saved) returns 0.
        let registers = frame(process.stack_ptr);
        unsafe {
            registers.write(0);
            registers.add(1).write(0);
            registers.add(2).write(0);
        }
        next.map(|_| process.stack_ptr)
    })
}

/// Closes `owner`; the app that launched it gets the screen back, its launch call returning
/// `result`.
pub fn exit(owner: u32, result: Result<u32, Error>) -> usize {
    cortex_m::interrupt::free(|cs| {
        let processes = &mut *PROCESSES.borrow(cs).borrow_mut();
        let mut parent = None;
        if let Some(slot) = owner.checked_sub(1) {
            parent = processes[slot as usize].take().and_then(|p| p.parent);
            memory::free_by_owner(owner);
        }
        let Some(id) = parent else {
            return create(processes, unsafe { &BOOT }, None).expect("app RAM unavailable");
        };

        // Restore the parent: its globals come back, and its launch call returns `result`.
        let process = processes[id as usize - 1].as_mut().unwrap();
        take_area(id).expect("no room to restore");
        let globals = process.globals as *const u8;
        unsafe { core::ptr::copy_nonoverlapping(globals, GLOBALS as *mut u8, PAGE_SIZE) };
        let _ = memory::free(id, process.globals);
        (process.state, process.globals) = (State::Running, 0);
        memory::map_pages(id);
        let (status, value) = match result {
            Ok(code) => (0, code),
            Err(error) => (error.code() as u32, 0),
        };
        let registers = frame(process.stack_ptr);
        unsafe {
            registers.write(status);
            registers.add(1).write(value);
            registers.add(2).write(0);
        }
        process.stack_ptr
    })
}

pub fn pause(owner: u32) -> Result<usize, Error> {
    cortex_m::interrupt::free(|cs| {
        let processes = &mut *PROCESSES.borrow(cs).borrow_mut();
        let Some(id) = processes[owner as usize - 1].and_then(|p| p.parent) else {
            return Err(Error::ENOENT);
        };

        // Save `owner`: its registers are on its stack (PSP, see svc.rs), its globals go to a
        // page of their own, and the area goes free.
        let globals = memory::alloc(owner, PAGE_SIZE, true)?;
        unsafe {
            core::ptr::copy_nonoverlapping(GLOBALS as *const u8, globals as *mut u8, PAGE_SIZE)
        };
        memory::free(owner, GLOBALS)?;
        let stack_ptr = cortex_m::register::psp::read() as usize;
        let me = processes[owner as usize - 1].as_mut().unwrap();
        *me = Process {
            state: State::Paused,
            stack_ptr,
            globals,
            ..*me
        };

        // Restore the parent: its globals come back, and its launch call returns 0.
        let process = processes[id as usize - 1].as_mut().unwrap();
        take_area(id).expect("no room to restore");
        let globals = process.globals as *const u8;
        unsafe { core::ptr::copy_nonoverlapping(globals, GLOBALS as *mut u8, PAGE_SIZE) };
        let _ = memory::free(id, process.globals);
        (process.state, process.globals) = (State::Running, 0);
        memory::map_pages(id);
        let registers = frame(process.stack_ptr);
        unsafe {
            registers.write(0);
            registers.add(1).write(0);
            registers.add(2).write(0);
        }
        Ok(process.stack_ptr)
    })
}

fn create(
    processes: &mut Processes,
    app: &'static AppHeader,
    parent: Option<u32>,
) -> Result<usize, Error> {
    let slot = processes
        .iter()
        .position(Option::is_none)
        .ok_or(Error::EBUSY)?;
    let id = slot as u32 + 1;
    if app.ram_size as usize > PAGE_SIZE {
        return Err(Error::ENOMEM);
    }
    take_area(id)?;
    let stack = memory::alloc(id, STACK_SIZE, false).inspect_err(|_| memory::free_by_owner(id))?;
    let stack_ptr = stack + STACK_SIZE - 32 - 36;
    let registers = stack_ptr as *mut u32;
    unsafe {
        registers.add(8).write(0xFFFF_FFFD); // EXC_RETURN: thread mode, its own stack
        registers.add(9 + 6).write((app.address() + app.entry) & !1); // PC
        registers.add(9 + 7).write(1 << 24); // xPSR: Thumb
    }
    let (state, globals) = (State::Running, 0);
    processes[slot] = Some(Process {
        app,
        state,
        stack_ptr,
        parent,
        globals,
    });
    memory::map_pages(id);
    Ok(stack_ptr)
}

fn take_area(id: u32) -> Result<(), Error> {
    let area = memory::alloc(id, AREA, true)?;
    assert_eq!(area, GLOBALS, "globals page was not free");
    Ok(())
}

pub fn frame(stack_ptr: usize) -> *mut u32 {
    let registers = stack_ptr as *mut u32;
    let fp = unsafe { registers.add(8).read() } & 0x10 == 0;
    unsafe { registers.add(if fp { 25 } else { 9 }) }
}

#[unsafe(naked)]
#[unsafe(export_name = "MemoryManagement")]
unsafe extern "C" fn fault_entry() {
    core::arch::naked_asm!(
        "and r0, lr, #0xC",
        "cmp r0, #0xC",
        "bne {kernel}",
        "bl {fault}",
        "ldmia r0!, {{r4-r11, lr}}",
        "tst lr, #0x10",
        "it eq",
        "vldmiaeq r0!, {{s16-s31}}",
        "msr PSP, r0",
        "bx lr",
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

extern "C" fn fault() -> usize {
    let scb = unsafe { &*SCB::PTR };
    unsafe { scb.cfsr.write(scb.cfsr.read()) };
    let (id, _) = current().expect("app fault without a process");
    exit(id, Err(Error::EFAULT))
}
