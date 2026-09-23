// SPDX-License-Identifier: GPL-3.0-only

use core::fmt;
use embedded_graphics::{
    mono_font::{ascii::FONT_8X13, MonoTextStyleBuilder},
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Drawable, Point, Size},
    primitives::Rectangle,
    text::{Baseline, Text},
};

pub const MAX_COLUMNS: usize = 40;
pub const MAX_ROWS: usize = 18;
const CELL_WIDTH: u32 = 8;
const CELL_HEIGHT: u32 = 13;

/// A fixed-size text viewport for direct LCD drawing.
pub struct Console {
    viewport: Rectangle,
    foreground: Rgb565,
    background: Rgb565,
    columns: usize,
    rows: usize,
    cursor_column: usize,
    cursor_row: usize,
    cells: [u8; MAX_COLUMNS * MAX_ROWS],
    dirty_from: usize,
    dirty_to: usize,
}

impl Console {
    pub fn new(area: Rectangle, foreground: Rgb565, background: Rgb565) -> Self {
        let columns = (area.size.width / CELL_WIDTH).min(MAX_COLUMNS as u32) as usize;
        let rows = (area.size.height / CELL_HEIGHT).min(MAX_ROWS as u32) as usize;
        let viewport = Rectangle::new(
            area.top_left,
            Size::new(columns as u32 * CELL_WIDTH, rows as u32 * CELL_HEIGHT),
        );

        Self {
            viewport,
            foreground,
            background,
            columns,
            rows,
            cursor_column: 0,
            cursor_row: 0,
            cells: [b' '; MAX_COLUMNS * MAX_ROWS],
            dirty_from: 0,
            dirty_to: rows,
        }
    }

    pub fn viewport(&self) -> Rectangle {
        self.viewport
    }

    pub const fn columns(&self) -> usize {
        self.columns
    }

    pub const fn rows(&self) -> usize {
        self.rows
    }

    pub fn writer<'a, T>(&'a mut self, target: &'a mut T) -> ConsoleWriter<'a, T>
    where
        T: DrawTarget<Color = Rgb565, Error = core::convert::Infallible>,
    {
        ConsoleWriter {
            console: self,
            target,
        }
    }

    fn write(&mut self, text: &str) {
        if self.columns == 0 || self.rows == 0 {
            return;
        }

        for character in text.chars() {
            match character {
                '\n' => self.new_line(),
                '\r' => self.cursor_column = 0,
                '\t' => {
                    let next_tab = (self.cursor_column / 4 + 1) * 4;
                    while self.cursor_column < next_tab {
                        self.put(b' ');
                    }
                }
                ' '..='~' => self.put(character as u8),
                _ => self.put(b'?'),
            }
        }
    }

    fn put(&mut self, character: u8) {
        if self.cursor_column == self.columns {
            self.new_line();
        }

        self.cells[self.cursor_row * self.columns + self.cursor_column] = character;
        self.cursor_column += 1;
        self.dirty_from = self.dirty_from.min(self.cursor_row);
        self.dirty_to = self.dirty_to.max(self.cursor_row + 1);
    }

    fn new_line(&mut self) {
        self.cursor_column = 0;
        if self.cursor_row + 1 < self.rows {
            self.cursor_row += 1;
            return;
        }

        self.cells
            .copy_within(self.columns..self.rows * self.columns, 0);
        self.cells[(self.rows - 1) * self.columns..self.rows * self.columns].fill(b' ');
        self.dirty_from = 0;
        self.dirty_to = self.rows;
    }

    fn redraw<T>(&mut self, target: &mut T) -> Result<(), core::convert::Infallible>
    where
        T: DrawTarget<Color = Rgb565, Error = core::convert::Infallible>,
    {
        let style = MonoTextStyleBuilder::new()
            .font(&FONT_8X13)
            .text_color(self.foreground)
            .background_color(self.background)
            .build();

        for row in self.dirty_from..self.dirty_to {
            let start = row * self.columns;
            let line = core::str::from_utf8(&self.cells[start..start + self.columns]).unwrap();
            Text::with_baseline(
                line,
                Point::new(
                    self.viewport.top_left.x,
                    self.viewport.top_left.y + row as i32 * CELL_HEIGHT as i32,
                ),
                style,
                Baseline::Top,
            )
            .draw(target)?;
        }
        self.dirty_from = self.rows;
        self.dirty_to = 0;
        Ok(())
    }
}

/// `core::fmt::Write` adapter returned by [`Console::writer`].
pub struct ConsoleWriter<'a, T> {
    console: &'a mut Console,
    target: &'a mut T,
}

impl<T> fmt::Write for ConsoleWriter<'_, T>
where
    T: DrawTarget<Color = Rgb565, Error = core::convert::Infallible>,
{
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.console.write(text);
        match self.console.redraw(self.target) {
            Ok(()) => Ok(()),
            Err(error) => match error {},
        }
    }
}

#[cfg(test)]
mod tests {
    use core::fmt::Write;
    use embedded_graphics::{
        mock_display::MockDisplay,
        pixelcolor::Rgb565,
        prelude::{Pixel, Point, RgbColor, Size},
        primitives::Rectangle,
    };

    use super::*;

    fn console(columns: u32, rows: u32) -> Console {
        Console::new(
            Rectangle::new(
                Point::zero(),
                Size::new(columns * CELL_WIDTH, rows * CELL_HEIGHT),
            ),
            Rgb565::WHITE,
            Rgb565::BLACK,
        )
    }

    fn write(console: &mut Console, text: &str) {
        let mut display = MockDisplay::<Rgb565>::new();
        display.set_allow_overdraw(true);
        console.writer(&mut display).write_str(text).unwrap();
    }

    #[test]
    fn geometry_rounds_down_to_complete_cells() {
        let console = Console::new(
            Rectangle::new(Point::new(3, 4), Size::new(320, 156)),
            Rgb565::WHITE,
            Rgb565::BLACK,
        );
        assert_eq!(console.columns(), 40);
        assert_eq!(console.rows(), 12);
        assert_eq!(console.viewport().size, Size::new(320, 156));

        let undersized = Console::new(
            Rectangle::new(Point::zero(), Size::new(7, 12)),
            Rgb565::WHITE,
            Rgb565::BLACK,
        );
        assert_eq!((undersized.columns(), undersized.rows()), (0, 0));
    }

    #[test]
    fn formatting_substitution_wrapping_and_tabs() {
        let mut tabbed = console(8, 2);
        write(&mut tabbed, "Aé\tB");
        assert_eq!(&tabbed.cells[..8], b"A?  B   ");

        let mut console = console(4, 2);
        write(&mut console, "abcdE\rZ");
        assert_eq!(&console.cells[..8], b"abcdZ   ");
    }

    #[test]
    fn oldest_row_is_evicted() {
        let mut console = console(2, 2);
        write(&mut console, "A\nB\nC");
        assert_eq!(&console.cells[..4], b"B C ");
    }

    #[test]
    fn append_leaves_other_rows_alone_but_scroll_repaints_them() {
        let mut console = console(2, 2);
        let mut display = MockDisplay::<Rgb565>::new();
        display.set_allow_overdraw(true);
        console.writer(&mut display).write_str("A").unwrap();

        let marker = Point::new(0, CELL_HEIGHT as i32);
        display.draw_iter([Pixel(marker, Rgb565::RED)]).unwrap();
        console.writer(&mut display).write_str("B").unwrap();
        assert_eq!(display.get_pixel(marker), Some(Rgb565::RED));

        console.writer(&mut display).write_str("\nC\nD").unwrap();
        assert_ne!(display.get_pixel(marker), Some(Rgb565::RED));
    }
}
