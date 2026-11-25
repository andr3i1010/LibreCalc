// SPDX-License-Identifier: GPL-3.0-only
use librecalc_abi::{Error, setting};
use n0120_hardware::TRAMPOLINE;

const LOG: u32 = 0x903F_0000;
const LOG_SIZE: u32 = 0x1_0000;
const LOG_SECTOR: u32 = 71;

fn records() -> impl Iterator<Item = [u32; 2]> {
    (LOG..LOG + LOG_SIZE)
        .step_by(8)
        .map(|address| unsafe { (address as *const [u32; 2]).read_volatile() })
        .take_while(|[key, _]| *key != u32::MAX)
}

pub fn get(key: u32) -> u32 {
    records()
        .filter(|[saved, _]| *saved == key)
        .last()
        .map_or(0, |[_, value]| value)
}

pub fn set(key: u32, value: u32) -> Result<(), Error> {
    if key as usize >= setting::COUNT {
        return Err(Error::EINVAL);
    }
    let used = records().count() as u32 * 8;
    if used < LOG_SIZE {
        return write(LOG + used, [key, value]);
    }
    let mut values: [u32; setting::COUNT] = core::array::from_fn(|other| get(other as u32));
    values[key as usize] = value;
    unsafe { ((*TRAMPOLINE).erase_sector)(LOG_SECTOR) };
    for ((address, key), value) in (LOG..).step_by(8).zip(0..).zip(values) {
        write(address, [key, value])?;
    }
    Ok(())
}

fn write(address: u32, record: [u32; 2]) -> Result<(), Error> {
    unsafe { ((*TRAMPOLINE).write_memory)(address as *mut u8, record.as_ptr().cast(), 8) };
    if unsafe { (address as *const [u32; 2]).read_volatile() } != record {
        return Err(Error::EIO);
    }
    Ok(())
}
