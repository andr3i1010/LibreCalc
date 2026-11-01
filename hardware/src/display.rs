// SPDX-License-Identifier: GPL-3.0-only

use core::{
    convert::Infallible,
    ptr::{read_volatile, write_volatile},
};
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{DrawTarget, IntoStorage, OriginDimensions, Pixel, Point, PointsIter, Size},
    primitives::Rectangle,
};

use crate::mcu::delay_us;

const DISPLAY_AREA: Rectangle = Rectangle::new(Point::zero(), Size::new(320, 240));

pub struct Display;

impl Display {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        delay_us(120_000);

        Self::write_command(0x01); // Software reset
        delay_us(5_000);

        Self::write_command(0x11); // Sleep out
        delay_us(120_000);

        Self::write_command(0x04); // Read display ID
        let read_data = || unsafe { read_volatile(0x6002_0000 as *const u16) };
        let _dummy = read_data();
        let id = (0..3).fold(0, |id, _| id << 8 | u32::from(read_data() as u8));
        Self::write(0x3A, &[0x55]); // 16-bit RGB565 pixels
        Self::write(0x35, &[0x00]); // Tearing effect output on
        Self::write(0x36, &[0xA0]); // Memory access control
        let (full, frame_rate, gamma) = match id {
            0x4E_4101 | 0x4E_4105 | 0x4E_4106 | 0x4E_4801 => (None, 0x1E, Some((0, 0))),
            0x4E_4102 => (Some(0), 0x05, Some((1, 1))),
            0x4E_4601 => (None, 0x05, Some((4, 4))),
            0x4E_4901 | 0x4E_4902 => (Some(1), 0x0F, Some((5, 6))),
            0x4E_5401 => (Some(0), 0x05, Some((2, 3))),
            _ => (None, 0x05, None),
        };
        if let Some(variant) = full {
            // Gate control (B7), VCOM (BB) and VRH (C3) per panel family.
            const VOLTAGES: [[u8; 3]; 2] = [[0x35, 0x3C, 0x10], [0x57, 0x1C, 0x0F]];
            let [gate, vcoms, vrh] = VOLTAGES[variant];
            Self::write(0xB2, &[0x0C, 0x0C, 0x00, 0x33, 0x33]); // Porch
            Self::write(0xC0, &[0x2C]);
            Self::write(0xC2, &[0x01]);
            Self::write(0xC4, &[0x20]);
            Self::write(0xD0, &[0xA4, 0xA1]); // Power control
            Self::write(0xB7, &[gate]);
            Self::write(0xBB, &[vcoms]);
            Self::write(0xC3, &[vrh]);
        }
        Self::write(0xC6, &[frame_rate]);
        if let Some((positive, negative)) = gamma {
            Self::write(0xE0, &GAMMA[positive]);
            Self::write(0xE1, &GAMMA[negative]);
        }

        Self::write_command(0x21); // Display inversion on
        Self::write_command(0x29); // Display on
        delay_us(20_000);
        Self
    }

    fn write(command: u8, data: &[u8]) {
        Self::write_command(command);
        for byte in data {
            Self::write_data(u16::from(*byte));
        }
    }

    fn write_command(command: u8) {
        unsafe {
            write_volatile(0x6000_0000 as *mut u16, u16::from(command));
        }
    }

    fn write_data(data: u16) {
        unsafe {
            write_volatile(0x6002_0000 as *mut u16, data);
        }
    }
}

const GAMMA: [[u8; 14]; 7] = [
    [
        0xA2, 0x0A, 0x11, 0x0A, 0x0C, 0x1A, 0x34, 0x22, 0x4D, 0x28, 0x15, 0x13, 0x29, 0x2D,
    ],
    [
        0xF0, 0x09, 0x0F, 0x08, 0x07, 0x03, 0x28, 0x11, 0x3C, 0x17, 0x12, 0x11, 0x28, 0x2D,
    ],
    [
        0xF0, 0x0D, 0x12, 0x09, 0x08, 0x14, 0x2D, 0x22, 0x43, 0x38, 0x17, 0x19, 0x2C, 0x30,
    ],
    [
        0xF0, 0x09, 0x0D, 0x0A, 0x09, 0x05, 0x29, 0x43, 0x40, 0x39, 0x18, 0x16, 0x2E, 0x2E,
    ],
    [
        0xC4, 0x0C, 0x13, 0x0C, 0x0E, 0x3C, 0x36, 0x44, 0x4F, 0x3A, 0x17, 0x15, 0x2B, 0x2F,
    ],
    [
        0xF0, 0x02, 0x08, 0x06, 0x07, 0x06, 0x31, 0x43, 0x48, 0x36, 0x12, 0x12, 0x2B, 0x34,
    ],
    [
        0xF0, 0x08, 0x0C, 0x0B, 0x09, 0x23, 0x31, 0x33, 0x47, 0x37, 0x14, 0x14, 0x2C, 0x32,
    ],
];

impl OriginDimensions for Display {
    fn size(&self) -> Size {
        DISPLAY_AREA.size
    }
}

impl DrawTarget for Display {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            self.fill_solid(&Rectangle::new(point, Size::new(1, 1)), color)?;
        }
        Ok(())
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let visible = area.intersection(&DISPLAY_AREA);
        let Some(end) = visible.bottom_right() else {
            return Ok(());
        };
        for (command, first, last) in [
            (0x2A, visible.top_left.x, end.x), // Column range
            (0x2B, visible.top_left.y, end.y), // Row range
        ] {
            let first = (first as u16).to_be_bytes();
            let last = (last as u16).to_be_bytes();
            Self::write(command, &[first[0], first[1], last[0], last[1]]);
        }
        Self::write_command(0x2C); // Pixel data
        for (point, color) in area.points().zip(colors) {
            if visible.contains(point) {
                Self::write_data(color.into_storage());
            }
        }
        Ok(())
    }
}
