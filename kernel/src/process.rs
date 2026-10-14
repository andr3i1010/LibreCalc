use core::cell::RefCell;
use cortex_m::interrupt::Mutex;
use librecalc_abi::Error;

pub enum State {
    Starting,
    Ready,
    Running,
    Exiting,
}

pub struct Process {
    pub id: u32,
    pub state: State,
    pub stack_start: usize,
    pub stack_end: usize,
    pub stack_ptr: usize,
}

const MAX_PROCESSES: usize = 2;

struct Processes {
    entries: [Option<Process>; MAX_PROCESSES],
    current: Option<u32>,
    next_id: u32,
}

static PROCESSES: Mutex<RefCell<Processes>> = Mutex::new(RefCell::new(Processes {
    entries: [const { None }; MAX_PROCESSES],
    current: None,
    next_id: 1,
}));

pub fn current_id() -> Option<u32> {
    cortex_m::interrupt::free(|cs| PROCESSES.borrow(cs).borrow().current)
}
pub fn create(stack_size: usize) -> Result<u32, Error> {
    cortex_m::interrupt::free(|cs| {
        let mut processes = PROCESSES.borrow(cs).borrow_mut();
        let slot = processes
            .entries
            .iter()
            .position(|entry| entry.is_none())
            .ok_or(Error::EBUSY)?;

        let id = processes.next_id;
        let next_id = id.checked_add(1).ok_or(Error::EOVERFLOW)?;

        let stack_size = stack_size
            .checked_next_multiple_of(crate::memory::PAGE_SIZE)
            .ok_or(Error::ENOMEM)?;
        let stack_start = crate::memory::alloc(id, stack_size)?;
        let stack_end = stack_start + stack_size;

        processes.entries[slot] = Some(Process {
            id,
            state: State::Starting,
            stack_start,
            stack_end,
            stack_ptr: stack_end,
        });
        
        processes.next_id = next_id;

        Ok(id)
    })
}
