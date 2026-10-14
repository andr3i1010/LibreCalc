use core::cell::RefCell;
use cortex_m::interrupt::Mutex;
use librecalc_abi::Error;

struct Allocation {
    owner: u32,
    address: usize,
    size: usize,
    capacity: usize,
}

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_COUNT: usize = 48;

struct Memory {
    start: usize,
    end: usize,
    allocations: [Option<Allocation>; PAGE_COUNT],
}

static MEMORY: Mutex<RefCell<Option<Memory>>> = Mutex::new(RefCell::new(None));

pub fn init() {
    unsafe extern "C" {
        static _user_ram_start: u8;
        static _user_ram_end: u8;
    }

    let start = (&raw const _user_ram_start) as usize;
    let end = (&raw const _user_ram_end) as usize;
    assert_ne!(start, 0, "start addr must not be zero");
    assert_eq!(
        start % PAGE_SIZE,
        0,
        "start addr must be aligned to page size"
    );
    assert_eq!(
        end.checked_sub(start),
        Some(PAGE_SIZE * PAGE_COUNT),
        "memory region must be exactly {} bytes",
        PAGE_SIZE * PAGE_COUNT
    );

    cortex_m::interrupt::free(|cs| {
        let mut memory = MEMORY.borrow(cs).borrow_mut();
        assert!(memory.is_none(), "memory already initialized");
        *memory = Some(Memory {
            start,
            end,
            allocations: core::array::from_fn(|_| None),
        });
    });
}

pub fn alloc(owner: u32, size: usize) -> Result<usize, Error> {
    with_memory(|memory| {
        if size == 0 || owner == 0 {
            return Err(Error::EINVAL);
        }

        let pages = size.div_ceil(PAGE_SIZE);

        if pages > PAGE_COUNT {
            return Err(Error::ENOMEM);
        }

        let capacity = pages * PAGE_SIZE;

        let slot = memory
            .allocations
            .iter()
            .position(|entry| entry.is_none())
            .ok_or(Error::ENOMEM)?;

        for address in (memory.start..=memory.end - capacity).step_by(PAGE_SIZE) {
            let candidate_end = address + capacity;

            let overlaps = memory.allocations.iter().flatten().any(|allocation| {
                let allocation_end = allocation.address + allocation.capacity;

                address < allocation_end && allocation.address < candidate_end
            });

            if overlaps {
                continue;
            }

            unsafe {
                core::ptr::write_bytes(address as *mut u8, 0, capacity);
            }

            memory.allocations[slot] = Some(Allocation {
                owner,
                address,
                size,
                capacity,
            });
            return Ok(address);
        }

        Err(Error::ENOMEM)
    })
}

pub fn free(owner: u32, address: usize) -> Result<(), Error> {
    with_memory(|memory| {
        if owner == 0 || address == 0 {
            return Err(Error::EINVAL);
        }

        let slot = memory
            .allocations
            .iter()
            .position(|entry| match entry {
                Some(allocation) => allocation.address == address,
                None => false,
            })
            .ok_or(Error::EINVAL)?;

        match &memory.allocations[slot] {
            Some(allocation) if allocation.owner == owner => {
                memory.allocations[slot] = None;
                Ok(())
            }
            _ => Err(Error::EACCES),
        }
    })
}

/// Release all allocations belonging to a process during cleanup.
pub fn free_by_owner(owner: u32) -> Result<(), Error> {
    with_memory(|memory| {
        if owner == 0 {
            return Err(Error::EINVAL);
        }
        for slot in 0..memory.allocations.len() {
            match &memory.allocations[slot] {
                Some(allocation) if allocation.owner == owner => memory.allocations[slot] = None,
                _ => {}
            }
        }
        Ok(())
    })
}

fn with_memory<R>(operation: impl FnOnce(&mut Memory) -> R) -> R {
    cortex_m::interrupt::free(|cs| {
        let mut memory = MEMORY.borrow(cs).borrow_mut();
        operation(memory.as_mut().expect("memory not initialized"))
    })
}
