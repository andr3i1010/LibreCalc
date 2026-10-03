// SPDX-License-Identifier: GPL-3.0-only

use core::{fmt::Write, panic::PanicInfo, ptr::read_volatile};

use cortex_m::peripheral::SCB;
use cortex_m_rt::{ExceptionFrame, exception};
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_9X18_BOLD},
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Drawable, Point, RgbColor, Size},
    primitives::Rectangle,
    text::{Baseline, Text},
};

use crate::{Console, Display};

const BODY: Rectangle = Rectangle::new(Point::new(0, 19), Size::new(320, 221));
const RAM_END: u32 = 0x2404_0000;
const TRACE_ADDRESSES: usize = 12;

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    cortex_m::interrupt::disable();

    let msp = cortex_m::register::msp::read();
    let psp = cortex_m::register::psp::read();
    let lr = cortex_m::register::lr::read();
    let (trace, trace_len) = scan_stack(msp);
    let mut display = crash_display();
    let mut writer = Console::new(&mut display, BODY, Rgb565::WHITE, Rgb565::BLACK);

    writer.write_limited(format_args!("Message: {}", info.message()), 3);
    if let Some(location) = info.location() {
        writer.write_limited(format_args!("At: {location}"), 2);
    }
    let _ = writeln!(writer, "MSP {msp:08X}  PSP {psp:08X}");
    let _ = writeln!(writer, "LR  {:08X}", lr & !1);
    halt(&mut writer, &trace[..trace_len])
}

/// Replaces the LCD contents with the Cortex-M exception frame and fault status.
#[exception]
unsafe fn HardFault(frame: &ExceptionFrame) -> ! {
    cortex_m::interrupt::disable();

    let stacked_sp = frame as *const ExceptionFrame as u32;
    let sp = stacked_sp
        .wrapping_add(core::mem::size_of::<ExceptionFrame>() as u32)
        .wrapping_add(if frame.xpsr() & (1 << 9) == 0 { 0 } else { 4 });
    let (trace, trace_len) = scan_stack(stacked_sp);
    let scb = unsafe { &*SCB::PTR };
    let cfsr = scb.cfsr.read();
    let mut display = crash_display();
    let mut writer = Console::new(&mut display, BODY, Rgb565::WHITE, Rgb565::BLACK);

    for (a, av, b, bv) in [
        ("R0", frame.r0(), "R1", frame.r1()),
        ("R2", frame.r2(), "R3", frame.r3()),
        ("R12", frame.r12(), "SP", sp),
        ("LR", frame.lr() & !1, "PC", frame.pc() & !1),
    ] {
        let _ = writeln!(writer, "{a:<4}{av:08X}  {b:<4}{bv:08X}");
    }
    let _ = writeln!(writer, "xPSR {:08X}", frame.xpsr());
    let _ = writeln!(writer, "CFSR {:08X}  HFSR {:08X}", cfsr, scb.hfsr.read());
    if cfsr & (1 << 7) != 0 {
        let _ = write!(writer, "MMFAR {:08X}  ", scb.mmfar.read());
    } else {
        let _ = write!(writer, "MMFAR --------  ");
    }
    if cfsr & (1 << 15) != 0 {
        let _ = writeln!(writer, "BFAR {:08X}", scb.bfar.read());
    } else {
        let _ = writeln!(writer, "BFAR --------");
    }
    let _ = writeln!(
        writer,
        "DFSR {:08X}  AFSR {:08X}",
        scb.dfsr.read(),
        scb.afsr.read()
    );
    let _ = writeln!(writer, "SHCSR {:08X}", scb.shcsr.read());
    halt(&mut writer, &trace[..trace_len])
}

fn scan_stack(sp: u32) -> ([u32; TRACE_ADDRESSES], usize) {
    let mut trace = [0; TRACE_ADDRESSES];
    let mut len = 0;
    if !(0x2400_0000..RAM_END).contains(&sp) || sp & 3 != 0 {
        return (trace, len);
    }

    for offset in 0..(((RAM_END - sp) / 4) as usize).min(96) {
        let value = unsafe { read_volatile((sp as *const u32).add(offset)) };
        let address = value & !1;
        if !(0x0800_0400..0x0808_0000).contains(&address)
            && !(0x9000_0400..0x9080_0000).contains(&address)
        {
            continue;
        }
        if trace[..len].contains(&address) {
            continue;
        }
        trace[len] = address;
        len += 1;
        if len == TRACE_ADDRESSES {
            break;
        }
    }
    (trace, len)
}

fn crash_display() -> Display {
    // Execution cannot resume, so the crash path can reuse the LCD MMIO.
    let mut display = Display;
    let _ = display.clear(Rgb565::BLACK);
    let _ = Text::with_baseline(
        "PANIC!",
        Point::new(2, 0),
        MonoTextStyle::new(&FONT_9X18_BOLD, Rgb565::RED),
        Baseline::Top,
    )
    .draw(&mut display);
    display
}

fn halt(writer: &mut impl Write, trace: &[u32]) -> ! {
    let _ = writeln!(writer, "Code addresses (addr2line):");
    if trace.is_empty() {
        let _ = writeln!(writer, "(none found)");
    }
    for (index, address) in trace.iter().enumerate() {
        let _ = write!(writer, "#{index:02} {address:08X}");
        let _ = writer.write_str(if index % 2 == 1 || index + 1 == trace.len() {
            "\n"
        } else {
            "  "
        });
    }
    let _ = write!(writer, "System halted.");
    loop {
        cortex_m::asm::wfi();
    }
}
