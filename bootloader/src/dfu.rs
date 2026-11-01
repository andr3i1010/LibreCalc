// SPDX-License-Identifier: GPL-3.0-only

use core::{cell::Cell, fmt::Write};
use cortex_m::interrupt::{self, Mutex};
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point, RgbColor, Size},
    primitives::Rectangle,
};
use n0120_hardware::{
    Console, Display, EXTERNAL_FLASH_END, EXTERNAL_FLASH_START, INTERNAL_FLASH_END,
    INTERNAL_FLASH_START, Key, STOCK_CPU_MHZ, erase_external_flash, erase_internal_flash,
    initialize_spi, initialize_usb, program_external_flash, program_internal_flash, read_keyboard,
    reset,
};
use usb_device::device::{StringDescriptors, UsbDeviceBuilder, UsbVidPid};
use usbd_dfu::{DFUClass, DFUManifestationError, DFUMemError, DFUMemIO};

struct Flash<const RECOVERY: bool> {
    external: bool,
    buffer: [u8; 128],
    stored: usize,
}

// Last external flash failure, shown on screen by the DFU loop (DFUClass hides the Flash).
static FAILURE: Mutex<Cell<Option<(&'static str, u32)>>> = Mutex::new(Cell::new(None));

fn report(error: &'static str, address: u32) {
    interrupt::free(|cs| FAILURE.borrow(cs).set(Some((error, address))));
}

impl<const RECOVERY: bool> DFUMemIO for Flash<RECOVERY> {
    const INITIAL_ADDRESS_POINTER: u32 = if RECOVERY { 0x0800_0000 } else { 0x9000_0000 };
    const MEM_INFO_STRING: &'static str = if RECOVERY {
        "@Flash/0x08000000/04*128Kg"
    } else {
        "@Flash/0x08000000/04*128Kg/0x90000000/08*004Kg,01*032Kg,127*064Kg"
    };
    const PROGRAM_TIME_MS: u32 = 10;
    const ERASE_TIME_MS: u32 = 5000;
    const FULL_ERASE_TIME_MS: u32 = 5000;

    fn read(&mut self, address: u32, length: usize) -> Result<&[u8], DFUMemError> {
        let end = address
            .checked_add(length as u32)
            .ok_or(DFUMemError::Address)?;
        if !((INTERNAL_FLASH_START..INTERNAL_FLASH_END).contains(&address)
            && end <= INTERNAL_FLASH_END
            || self.external
                && (EXTERNAL_FLASH_START..EXTERNAL_FLASH_END).contains(&address)
                && end <= EXTERNAL_FLASH_END)
        {
            return Err(DFUMemError::Address);
        }
        Ok(unsafe { core::slice::from_raw_parts(address as *const u8, length) })
    }

    fn store_write_buffer(&mut self, source: &[u8]) -> Result<(), ()> {
        if source.len() > self.buffer.len() {
            return Err(());
        }
        self.buffer[..source.len()].copy_from_slice(source);
        self.stored = source.len();
        Ok(())
    }

    fn program(&mut self, address: u32, length: usize) -> Result<(), DFUMemError> {
        if length != self.stored {
            return Err(DFUMemError::Write);
        }
        let end = address
            .checked_add(length as u32)
            .ok_or(DFUMemError::Address)?;
        if length == 0 {
            return Err(DFUMemError::Address);
        }
        let data = &self.buffer[..length];
        if (INTERNAL_FLASH_START..INTERNAL_FLASH_END).contains(&address)
            && end <= INTERNAL_FLASH_END
        {
            program_internal_flash(address, data).map_err(|_| DFUMemError::Prog)?;
        } else if self.external
            && (EXTERNAL_FLASH_START..EXTERNAL_FLASH_END).contains(&address)
            && end <= EXTERNAL_FLASH_END
        {
            program_external_flash::<true>(address, data).map_err(|error| {
                report(error, address);
                DFUMemError::Prog
            })?;
            let programmed = unsafe { core::slice::from_raw_parts(address as *const u8, length) };
            if programmed != data {
                report("program verify failed", address);
                return Err(DFUMemError::Verify);
            }
        } else {
            return Err(DFUMemError::Address);
        }
        self.stored = 0;
        Ok(())
    }

    fn erase(&mut self, address: u32) -> Result<(), DFUMemError> {
        if (INTERNAL_FLASH_START..INTERNAL_FLASH_END).contains(&address) {
            erase_internal_flash(address).map_err(|_| DFUMemError::Erase)
        } else if self.external && (EXTERNAL_FLASH_START..EXTERNAL_FLASH_END).contains(&address) {
            let result = erase_external_flash::<true>(address).and_then(|_| {
                let start = unsafe { core::slice::from_raw_parts(address as *const u8, 16) };
                start
                    .iter()
                    .all(|byte| *byte == 0xFF)
                    .then_some(())
                    .ok_or("erase had no effect")
            });
            result.map_err(|error| {
                report(error, address);
                DFUMemError::Erase
            })
        } else {
            Err(DFUMemError::Address)
        }
    }

    fn erase_all(&mut self) -> Result<(), DFUMemError> {
        Err(DFUMemError::Erase)
    }

    fn manifestation(&mut self) -> Result<(), DFUManifestationError> {
        Ok(())
    }
}

pub fn run(display: &mut Display) -> ! {
    let external = initialize_spi().is_ok();
    let allocator = initialize_usb(STOCK_CPU_MHZ * 500_000);
    let mut class = DFUClass::new(
        &allocator,
        Flash::<false> {
            external,
            buffer: [0; 128],
            stored: 0,
        },
    );
    display.clear(Rgb565::BLACK).unwrap();
    let mut log = Console::new(
        display,
        Rectangle::new(Point::zero(), Size::new(320, 240)),
        Rgb565::WHITE,
        Rgb565::BLACK,
    );
    writeln!(log, "DFU mode\nUSB 0483:a291").unwrap();
    if !external {
        writeln!(log, "External flash unavailable").unwrap();
    }
    writeln!(log, "Press BACK to reboot").unwrap();
    // Building connects USB; finish the slow LCD writes before the host can enumerate.
    let mut device = UsbDeviceBuilder::new(&allocator, UsbVidPid(0x0483, 0xA291))
        .strings(&[StringDescriptors::default()
            .manufacturer("NumCrack")
            .product("LibreCalc Bootloader")])
        .unwrap()
        .max_packet_size_0(64)
        .unwrap()
        .build();

    loop {
        for _ in 0..4096 {
            device.poll(&mut [&mut class]);
        }
        if let Some((error, address)) = interrupt::free(|cs| FAILURE.borrow(cs).take()) {
            writeln!(log, "{:08X}: {}", address, error).unwrap();
        }
        if Key::Back.is_pressed(read_keyboard()) {
            reset();
        }
    }
}

pub fn recovery() -> ! {
    let allocator = initialize_usb(64_000_000);
    let mut class = DFUClass::new(
        &allocator,
        Flash::<true> {
            external: false,
            buffer: [0; 128],
            stored: 0,
        },
    );
    let mut device = UsbDeviceBuilder::new(&allocator, UsbVidPid(0x0483, 0xA291))
        .strings(&[StringDescriptors::default()
            .manufacturer("NumCrack")
            .product("LibreCalc Recovery")])
        .unwrap()
        .max_packet_size_0(64)
        .unwrap()
        .build();
    loop {
        device.poll(&mut [&mut class]);
    }
}
