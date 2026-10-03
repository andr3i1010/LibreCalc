// SPDX-License-Identifier: GPL-3.0-only

use core::{convert::Infallible, ptr::write_volatile};
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{DrawTarget, IntoStorage, OriginDimensions, Pixel, Point, PointsIter, Size},
    primitives::Rectangle,
};

use crate::mcu::delay_us;

pub(crate) const HEIGHT: u16 = 240;

const DISPLAY_AREA: Rectangle = Rectangle::new(Point::zero(), Size::new(320, HEIGHT as u32));

pub struct Display;

impl Display {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        delay_ms(120);

        Self::write_command(0x01); // Software reset
        delay_ms(5);

        Self::write_command(0x11); // Sleep out
        delay_ms(120);

        Self::write_command(0x3A); // 16-bit RGB565 pixels
        Self::write_data(0x55);

        Self::write_command(0x36); // Memory access control
        Self::write_data(0xA0);

        Self::write_command(0x21); // Display inversion on
        Self::write_command(0x29); // Display on
        delay_ms(20);
        Self
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
            Self::write_command(command);
            for byte in (first as u16)
                .to_be_bytes()
                .into_iter()
                .chain((last as u16).to_be_bytes())
            {
                Self::write_data(u16::from(byte));
            }
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

fn delay_ms(milliseconds: u32) {
    delay_us(milliseconds.saturating_mul(1_000));
}
