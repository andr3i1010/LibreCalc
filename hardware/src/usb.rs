// SPDX-License-Identifier: GPL-3.0-only

use stm32h7::stm32h725;
use synopsys_usb_otg::UsbBus;
use usb_device::bus::UsbBusAllocator;

use crate::mcu::{GPIOA, set_field};

pub struct Usb {
    ahb_frequency_hz: u32,
}

unsafe impl synopsys_usb_otg::UsbPeripheral for Usb {
    const REGISTERS: *const () = stm32h725::OTG1_HS_GLOBAL::PTR as *const ();
    const HIGH_SPEED: bool = true;
    const FIFO_DEPTH_WORDS: usize = 1024;
    const ENDPOINT_COUNT: usize = 9;

    fn enable() {
        let rcc = unsafe { &*stm32h725::RCC::ptr() };
        rcc.ahb1enr().modify(|_, w| w.usb1otghsen().enabled());
        let _ = rcc.ahb1enr().read().bits();
    }

    fn ahb_frequency_hz(&self) -> u32 {
        self.ahb_frequency_hz
    }
}

pub fn initialize_usb_hardware() {
    let rcc = unsafe { &*stm32h725::RCC::ptr() };
    let crs = unsafe { &*stm32h725::CRS::ptr() };
    let pwr = unsafe { &*stm32h725::PWR::ptr() };

    rcc.cr().modify(|_, w| w.hsi48on().set_bit());
    while rcc.cr().read().hsi48rdy().bit_is_clear() {}
    rcc.d2ccip2r().modify(|_, w| w.usbsel().hsi48());
    rcc.apb1henr().modify(|_, w| w.crsen().enabled());
    let _ = rcc.apb1henr().read().bits();
    crs.cr()
        .modify(|_, w| w.autotrimen().set_bit().cen().set_bit());

    pwr.cr3().modify(|_, w| w.usb33den().set_bit());
    while pwr.cr3().read().usb33rdy().bit_is_clear() {}

    rcc.ahb4enr().modify(|_, w| w.gpioaen().enabled());
    let _ = rcc.ahb4enr().read().bits();
    set_field(GPIOA, 0x24, 12, 8, 0xAA);
    set_field(GPIOA, 0x00, 22, 4, 0b1010);
    set_field(GPIOA, 0x00, 18, 2, 0);
    set_field(GPIOA, 0x0C, 18, 2, 0);
    <Usb as synopsys_usb_otg::UsbPeripheral>::enable();
}

// Call once for the singleton memory; building a device from this allocator connects USB.
pub fn initialize_usb(ahb_frequency_hz: u32) -> UsbBusAllocator<UsbBus<Usb>> {
    initialize_usb_hardware();

    let memory = cortex_m::singleton!(: [u32; 1024] = [0; 1024]).unwrap();
    UsbBus::new(Usb { ahb_frequency_hz }, memory)
}
