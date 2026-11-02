use core::{
    alloc::{GlobalAlloc, Layout},
    cell::RefCell,
    ops::Range,
};
use cortex_m::interrupt::Mutex;
use librecalc_abi::Error;

struct Allocation {
    owner: u32,
    address: usize,
    size: usize,
    capacity: usize,
    heap: bool,
}

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_COUNT: usize = 124;

struct Memory {
    regions: [Range<usize>; 6],
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

    let regions = [
        start..end,               // AXI SRAM above the kernel
        0x2000_0000..0x2002_0000, // DTCM
        0x0000_1000..0x0001_0000, // ITCM, without page 0 so no allocation is a null pointer
        0x3000_0000..0x3000_8000, // SRAM1, SRAM2
        0x3800_0000..0x3800_4000, // SRAM4
        0x3880_0000..0x3880_1000, // Backup SRAM
    ];
    assert_eq!(
        regions.iter().map(|region| region.len()).sum::<usize>(),
        PAGE_SIZE * PAGE_COUNT,
        "memory regions must be exactly {} bytes",
        PAGE_SIZE * PAGE_COUNT
    );

    n0120_hardware::enable_backup_sram();
    // Fail at boot rather than in some later alloc if a block is missing or mirrored.
    for region in &regions {
        for address in region.clone().step_by(4) {
            unsafe { (address as *mut u32).write_volatile(address as u32) };
        }
        for address in region.clone().step_by(4) {
            let word = unsafe { (address as *const u32).read_volatile() };
            assert_eq!(word, address as u32, "RAM test failed at {address:#010x}");
        }
    }

    cortex_m::interrupt::free(|cs| {
        let mut memory = MEMORY.borrow(cs).borrow_mut();
        assert!(memory.is_none(), "memory already initialized");
        *memory = Some(Memory {
            regions,
            allocations: core::array::from_fn(|_| None),
        });
    });
}

pub fn alloc(owner: u32, size: usize) -> Result<usize, Error> {
    if owner == 0 {
        return Err(Error::EINVAL);
    }
    allocate(owner, size, false)
}

pub fn alloc_heap(owner: u32, size: usize) -> Result<usize, Error> {
    allocate(owner, size, true)
}

fn allocate(owner: u32, size: usize, heap: bool) -> Result<usize, Error> {
    with_memory(|memory| {
        if size == 0 {
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

        let candidates = memory
            .regions
            .iter()
            .filter(|region| region.len() >= capacity)
            .flat_map(|region| (region.start..=region.end - capacity).step_by(PAGE_SIZE));

        for address in candidates {
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
                heap,
            });
            return Ok(address);
        }

        Err(Error::ENOMEM)
    })
}

pub fn free(owner: u32, address: usize) -> Result<(), Error> {
    with_memory(|memory| {
        if address == 0 {
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
            Some(allocation) if allocation.owner == owner && allocation.heap => {
                memory.allocations[slot] = None;
                Ok(())
            }
            _ => Err(Error::EACCES),
        }
    })
}

/// Release all allocations belonging to a process during cleanup.
pub fn free_by_owner(owner: u32) -> Result<(), Error> {
    if owner == 0 {
        return Err(Error::EINVAL);
    }
    with_memory(|memory| {
        for slot in 0..memory.allocations.len() {
            match &memory.allocations[slot] {
                Some(allocation) if allocation.owner == owner => memory.allocations[slot] = None,
                _ => {}
            }
        }
        Ok(())
    })
}

pub struct KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // shortcut: heap blocks use whole pages; add suballocation when app workloads need it.
        unsafe { librecalc_abi::heap::allocate(layout, |size| alloc_heap(0, size as usize)) }
    }

    unsafe fn dealloc(&self, address: *mut u8, _: Layout) {
        let _ = free(0, unsafe { librecalc_abi::heap::allocation_base(address) });
    }
}

fn with_memory<R>(operation: impl FnOnce(&mut Memory) -> R) -> R {
    cortex_m::interrupt::free(|cs| {
        let mut memory = MEMORY.borrow(cs).borrow_mut();
        operation(memory.as_mut().expect("memory not initialized"))
    })
}
