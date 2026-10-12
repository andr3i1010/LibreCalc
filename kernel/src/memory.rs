use librecalc_abi::Error;

pub struct Allocation {
    owner: u32,
    address: usize,
    size: usize,
    capacity: usize,
}

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_COUNT: usize = 48;

pub struct Memory {
    start: usize,
    end: usize,
    allocations: [Option<Allocation>; PAGE_COUNT],
}

impl Memory {
    pub unsafe fn new(start: usize, end: usize) -> Self {
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
        Self {
            start,
            end,
            allocations: core::array::from_fn(|_| None),
        }
    }
    pub fn alloc(&mut self, owner: u32, size: usize) -> Result<usize, Error> {
        if size == 0 || owner == 0 {
            return Err(Error::EINVAL);
        }

        let pages = size.div_ceil(PAGE_SIZE);

        if pages > PAGE_COUNT {
            return Err(Error::ENOMEM);
        }

        let capacity = pages * PAGE_SIZE;

        let slot = self
            .allocations
            .iter()
            .position(|entry| entry.is_none())
            .ok_or(Error::ENOMEM)?;

        for address in (self.start..=self.end - capacity).step_by(PAGE_SIZE) {
            let candidate_end = address + capacity;

            let overlaps = self.allocations.iter().flatten().any(|allocation| {
                let allocation_end = allocation.address + allocation.capacity;

                address < allocation_end && allocation.address < candidate_end
            });

            if overlaps {
                continue;
            }

            unsafe {
                core::ptr::write_bytes(address as *mut u8, 0, capacity);
            }

            self.allocations[slot] = Some(Allocation {
                owner, 
                address, 
                size, 
                capacity
            });
            return Ok(address);
        }

        Err(Error::ENOMEM)
    }
    pub fn free(&mut self, owner: u32, address: usize) -> Result<(), Error> {
        if owner == 0 || address == 0 {
            return Err(Error::EINVAL);
        }

        let slot = self.allocations
            .iter()
            .position(|entry| match entry {
                Some(allocation) => allocation.address == address,
                None => false
            })
            .ok_or(Error::EINVAL)?;

        match &self.allocations[slot] {
            Some(allocation) if allocation.owner == owner => {
                self.allocations[slot] = None;
                Ok(())
            }
            _ => Err(Error::EACCES)
        }
    }
    pub fn free_by_owner(&mut self, owner: u32) -> Result<(), Error> {
        if owner == 0 {
            return Err(Error::EINVAL)
        }
        for slot in 0..self.allocations.len() {
            match &self.allocations[slot] {
                Some(allocation) if allocation.owner == owner => {
                    self.allocations[slot] = None
                }
                _ => {}
            }
        }
        Ok(())
    }
}
