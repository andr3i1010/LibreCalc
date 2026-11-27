use core::{cell::RefCell, ops::Range};
use cortex_m::{interrupt::Mutex, peripheral::MPU};
use librecalc_abi::Error;
use n0120_hardware::{AXI_SRAM, DTCM, EXTERNAL_FLASH_END, EXTERNAL_FLASH_START, ITCM, SRAM1_2};

struct Allocation {
    owner: u32,
    address: usize,
    capacity: usize,
    heap: bool,
}

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_COUNT: usize = 112;
pub const APP_REGION_COUNT: usize = 14;

pub const APP_RAM: [Range<usize>; 4] = [
    AXI_SRAM.start as usize..AXI_SRAM.end as usize,
    DTCM.start as usize + 0x1_0000..DTCM.end as usize,
    SRAM1_2.start as usize..SRAM1_2.end as usize,
    ITCM.start as usize + 0x8000..ITCM.end as usize,
];

struct Memory {
    allocations: [Option<Allocation>; PAGE_COUNT],
}

static MEMORY: Mutex<RefCell<Option<Memory>>> = Mutex::new(RefCell::new(None));
fn with_memory<R>(operation: impl FnOnce(&mut Memory) -> R) -> R {
    cortex_m::interrupt::free(|cs| {
        let mut memory = MEMORY.borrow(cs).borrow_mut();
        operation(memory.as_mut().expect("memory not initialized"))
    })
}

pub fn init() {
    assert_eq!(
        APP_RAM.iter().map(|region| region.len()).sum::<usize>(),
        PAGE_SIZE * PAGE_COUNT,
        "memory regions must be exactly {} bytes",
        PAGE_SIZE * PAGE_COUNT
    );
    assert_eq!(app_regions().count(), APP_REGION_COUNT);

    for region in &APP_RAM {
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
            allocations: core::array::from_fn(|_| None),
        });
    });

    const XN: u32 = 1 << 28;
    const READ_WRITE: u32 = 0b011 << 24;
    const READ_ONLY: u32 = 0b110 << 24;
    const WRITE_BACK: u32 = 0b001 << 19 | 1 << 17 | 1 << 16;
    const WRITE_THROUGH: u32 = 1 << 17;
    const fn size(bytes: u32) -> u32 {
        (bytes.trailing_zeros() - 1) << 1 | 1
    }

    let mpu = unsafe { &*MPU::PTR };
    assert_eq!(mpu._type.read() >> 8 & 0xFF, 16, "expected 16 MPU regions");
    cortex_m::asm::dsb();
    unsafe {
        mpu.ctrl.write(0);
        for number in 1..16 {
            mpu.rnr.write(number);
            mpu.rasr.write(0);
        }
        for (number, start) in (1..).zip(app_regions()) {
            mpu.rnr.write(number);
            mpu.rbar.write(start as u32);
            mpu.rasr
                .write(READ_WRITE | XN | WRITE_BACK | (0xFF << 8) | size(32 * 1024));
        }
        mpu.rnr.write(APP_REGION_COUNT as u32 + 1);
        mpu.rbar.write(EXTERNAL_FLASH_START);
        mpu.rasr
            .write(READ_ONLY | WRITE_THROUGH | size(EXTERNAL_FLASH_END - EXTERNAL_FLASH_START));
        mpu.ctrl.write(5);
    }
    cortex_m::asm::dsb();
    cortex_m::asm::isb();
}

pub fn app_regions() -> impl Iterator<Item = usize> {
    APP_RAM
        .iter()
        .flat_map(|region| region.clone().step_by(8 * PAGE_SIZE))
}

pub fn map_pages(owner: u32) {
    let mpu = unsafe { &*MPU::PTR };
    cortex_m::asm::dsb();
    with_memory(|memory| unsafe {
        mpu.ctrl.write(0);
        for (number, start) in (1..).zip(app_regions()) {
            let hidden = (0..8)
                .filter(|page| !owned(memory, owner, start + page * PAGE_SIZE))
                .fold(0, |mask, page| mask | 1 << page);
            mpu.rnr.write(number);
            mpu.rasr.write(mpu.rasr.read() & !(0xFF << 8) | hidden << 8);
        }
        mpu.ctrl.write(5);
    });
    cortex_m::asm::dsb();
    cortex_m::asm::isb();
}

pub fn owns(owner: u32, address: usize, length: usize) -> bool {
    let Some(end) = address.checked_add(length) else {
        return false;
    };
    with_memory(|memory| {
        (address & !(PAGE_SIZE - 1)..end)
            .step_by(PAGE_SIZE)
            .all(|page| owned(memory, owner, page))
    })
}

fn owned(memory: &Memory, owner: u32, page: usize) -> bool {
    memory.allocations.iter().flatten().any(|allocation| {
        allocation.owner == owner
            && (allocation.address..allocation.address + allocation.capacity).contains(&page)
    })
}

pub fn alloc(owner: u32, size: usize, heap: bool) -> Result<usize, Error> {
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

        let candidates = APP_RAM
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

pub fn free_by_owner(owner: u32) {
    with_memory(|memory| {
        for slot in &mut memory.allocations {
            if slot
                .as_ref()
                .is_some_and(|allocation| allocation.owner == owner)
            {
                *slot = None;
            }
        }
    })
}
