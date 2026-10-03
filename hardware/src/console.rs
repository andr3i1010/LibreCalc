// SPDX-License-Identifier: GPL-3.0-only

use core::{convert::Infallible, fmt};
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_8X13},
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Drawable, Point},
    primitives::Rectangle,
    text::{Baseline, Text},
};

const MAX_COLUMNS: usize = 40;
const MAX_ROWS: usize = 18;
const CELL_HEIGHT: u32 = 13;

/// A fixed-size text viewport for direct LCD drawing.
pub struct Console<'a, T> {
    target: &'a mut T,
    position: Point,
    style: MonoTextStyle<'static, Rgb565>,
    columns: usize,
    rows: usize,
    cursor_column: usize,
    cursor_row: usize,
    limit: usize,
    cells: [u8; MAX_COLUMNS * MAX_ROWS],
    dirty_from: usize,
    dirty_to: usize,
}

impl<'a, T> Console<'a, T> {
    pub fn new(target: &'a mut T, area: Rectangle, foreground: Rgb565, background: Rgb565) -> Self {
        let columns = (area.size.width / 8).min(MAX_COLUMNS as u32) as usize;
        let rows = (area.size.height / CELL_HEIGHT).min(MAX_ROWS as u32) as usize;
        let mut style = MonoTextStyle::new(&FONT_8X13, foreground);
        style.background_color = Some(background);
        Self {
            target,
            position: area.top_left,
            style,
            columns,
            rows,
            cursor_column: 0,
            cursor_row: 0,
            limit: rows,
            cells: [b' '; MAX_COLUMNS * MAX_ROWS],
            dirty_from: 0,
            dirty_to: rows,
        }
    }

    #[cfg(target_os = "none")]
    pub(crate) fn write_limited(&mut self, arguments: fmt::Arguments<'_>, rows: usize)
    where
        T: DrawTarget<Color = Rgb565, Error = Infallible>,
    {
        self.limit = (self.cursor_row + rows).min(self.rows);
        let _ = fmt::Write::write_fmt(self, arguments);
        if self.cursor_column != 0 {
            self.cursor_row += 1;
        }
        self.cursor_column = 0;
        self.limit = self.rows;
    }
}

impl<T> fmt::Write for Console<'_, T>
where
    T: DrawTarget<Color = Rgb565, Error = Infallible>,
{
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self.columns == 0 || self.rows == 0 || self.cursor_row >= self.limit {
            return Ok(());
        }

        for character in text.chars() {
            if self.cursor_row >= self.limit {
                break;
            }
            if character == '\r' {
                self.cursor_column = 0;
                continue;
            }
            let count = if character == '\t' {
                4 - self.cursor_column % 4
            } else {
                1
            };
            for _ in 0..count {
                if character == '\n' || self.cursor_column == self.columns {
                    self.cursor_column = 0;
                    if self.cursor_row + 1 < self.rows {
                        self.cursor_row += 1;
                    } else {
                        self.cells
                            .copy_within(self.columns..self.rows * self.columns, 0);
                        self.cells[(self.rows - 1) * self.columns..self.rows * self.columns]
                            .fill(b' ');
                        self.dirty_from = 0;
                        self.dirty_to = self.rows;
                    }
                }
                if character != '\n' && self.cursor_row < self.limit {
                    self.cells[self.cursor_row * self.columns + self.cursor_column] =
                        match character {
                            '\t' => b' ',
                            ' '..='~' => character as u8,
                            _ => b'?',
                        };
                    self.cursor_column += 1;
                    self.dirty_from = self.dirty_from.min(self.cursor_row);
                    self.dirty_to = self.dirty_to.max(self.cursor_row + 1);
                }
            }
        }

        for row in self.dirty_from..self.dirty_to {
            let start = row * self.columns;
            let line = core::str::from_utf8(&self.cells[start..start + self.columns]).unwrap();
            Text::with_baseline(
                line,
                self.position + Point::new(0, row as i32 * CELL_HEIGHT as i32),
                self.style,
                Baseline::Top,
            )
            .draw(self.target)
            .unwrap();
        }
        self.dirty_from = self.rows;
        self.dirty_to = 0;
        Ok(())
    }
}
