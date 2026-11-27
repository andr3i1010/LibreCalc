// SPDX-License-Identifier: GPL-3.0-only
#![no_std]

extern crate alloc;

use alloc::string::String;
use librecalc_abi::{CHARACTERS, Rect};

mod ui;
mod widgets;
pub use ui::{Align, Response, Theme, Ui, run};
pub use widgets::Change;

include!(concat!(env!("OUT_DIR"), "/assets.rs"));
pub const fn app_icon(icon: Icon, color: u16) -> u32 {
    icon as u32 | (color as u32) << 16
}

const TABLE: *const Table = librecalc_abi::LIBRARY as *const Table;

macro_rules! library {
    ($($(#[$attr:meta])* fn $name:ident($($arg:ident: $ty:ty),*) $(-> $ret:ty)? $body:block)*) => {
        #[repr(C)]
        pub struct Table {
            $(pub $name: fn($($ty),*) $(-> $ret)?,)*
        }

        #[unsafe(no_mangle)]
        #[unsafe(link_section = ".graphite_ui_table")]
        static GRAPHITE_UI_TABLE: Table = Table {
            $($name: {
                fn $name($($arg: $ty),*) $(-> $ret)? $body
                $name
            },)*
        };

        $(
            $(#[$attr])*
            pub fn $name($($arg: $ty),*) $(-> $ret)? {
                unsafe { ((*TABLE).$name)($($arg),*) }
            }
        )*
    };
}

struct Canvas<'a> {
    area: Rect,
    clip: Rect,
    pixels: &'a mut [u16],
}

impl Canvas<'_> {
    fn blend(&mut self, x: i32, y: i32, color: u16, alpha: u32) {
        let inside = |value: i32, start: i16, length: i16| {
            (i32::from(start)..i32::from(start) + i32::from(length)).contains(&value)
        };
        let (area, clip) = (self.area, self.clip);
        if alpha == 0
            || !inside(x, area.x, area.width)
            || !inside(y, area.y, area.height)
            || !inside(x, clip.x, clip.width)
            || !inside(y, clip.y, clip.height)
        {
            return;
        }
        let index = (y - i32::from(area.y)) * i32::from(area.width) + x - i32::from(area.x);
        let Some(pixel) = self.pixels.get_mut(index as usize) else {
            return;
        };
        let channel = |shift: u32, mask: u32| {
            let (under, over) = (
                u32::from(*pixel) >> shift & mask,
                u32::from(color) >> shift & mask,
            );
            ((over * alpha + under * (255 - alpha) + 127) / 255) << shift
        };
        *pixel = (channel(11, 31) | channel(5, 63) | channel(0, 31)) as u16;
    }

    fn glyph(&mut self, font: &FontData, glyph: &Glyph, x: i32, y: i32, color: u16) -> i32 {
        let (width, bits) = (u32::from(glyph.width), &font.bits[glyph.start as usize..]);
        let (left, top) = (x + i32::from(glyph.left), y + i32::from(glyph.top));
        for index in 0..width * u32::from(glyph.height) {
            let nibble = bits[index as usize / 2] >> (index % 2 * 4) & 15;
            let (column, row) = ((index % width) as i32, (index / width) as i32);
            self.blend(left + column, top + row, color, u32::from(nibble) * 17);
        }
        glyph.advance.into()
    }
}

fn coverage(rect: Rect, radius: u16, x: i32, y: i32) -> u32 {
    let r = i32::from(radius);
    let (left, top) = (i32::from(rect.x), i32::from(rect.y));
    let (right, bottom) = (left + i32::from(rect.width), top + i32::from(rect.height));
    if x < left || y < top || x >= right || y >= bottom {
        return 0;
    }
    let cx = if x < left + r {
        left + r
    } else if x >= right - r {
        right - r
    } else {
        return 16;
    };
    let cy = if y < top + r {
        top + r
    } else if y >= bottom - r {
        bottom - r
    } else {
        return 16;
    };
    (0..16)
        .filter(|sample| {
            let dx = x * 8 + 1 + sample % 4 * 2 - cx * 8;
            let dy = y * 8 + 1 + sample / 4 * 2 - cy * 8;
            dx * dx + dy * dy <= r * r * 64
        })
        .count() as u32
}

struct Glyph {
    width: u8,
    height: u8,
    left: i8,
    top: i8,
    advance: u8,
    start: u32,
}

struct FontData {
    line_height: u8,
    glyphs: &'static [Glyph],
    bits: &'static [u8],
}

fn glyph(font: &FontData, character: char) -> &Glyph {
    let index = (character as usize).wrapping_sub(0x20);
    font.glyphs
        .get(index)
        .unwrap_or(&font.glyphs['?' as usize - 0x20])
}

#[derive(PartialEq)]
pub struct Command {
    pub rect: Rect,
    pub clip: Rect,
    pub color: u16,
    pub shape: Shape,
}

#[derive(PartialEq)]
pub enum Shape {
    Fill {
        radius: u16,
        alpha: u8,
    },
    Outline {
        radius: u16,
        alpha: u8,
    },
    Text(String, Font),
    Icon(Icon),
}

library! {
    fn text_size(text: &str, font: Font) -> Rect {
        let font = &FONTS[font as usize];
        let width = text.chars().map(|character| i16::from(glyph(font, character).advance)).sum();
        Rect { width, height: font.line_height.into(), ..Rect::default() }
    }

    fn draw(commands: &[Command], pixels: &mut [u16], area: Rect) {
        let mut canvas = Canvas { area, clip: Rect::default(), pixels };
        let (top, bottom) = (area.y, area.y + area.height);
        for &Command { rect, clip, color, ref shape } in commands {
            if rect.y >= bottom || rect.y + rect.height <= top {
                continue;
            }
            canvas.clip = clip;
            match *shape {
                Shape::Fill { radius, alpha } | Shape::Outline { radius, alpha } => {
                    let hole = match shape {
                        Shape::Outline { .. } => Rect { x: rect.x + 1, y: rect.y + 1, width: rect.width - 2, height: rect.height - 2 },
                        _ => Rect::default(),
                    };
                    for y in rect.y.max(top)..(rect.y + rect.height).min(bottom) {
                        for x in rect.x..rect.x + rect.width {
                            let (x, y) = (i32::from(x), i32::from(y));
                            let cover = coverage(rect, radius, x, y).saturating_sub(coverage(hole, radius.saturating_sub(1), x, y));
                            canvas.blend(x, y, color, u32::from(alpha) * cover / 16);
                        }
                    }
                }
                Shape::Text(ref text, font) => {
                    let font = &FONTS[font as usize];
                    let mut pen = i32::from(rect.x);
                    for character in text.chars() {
                        pen += canvas.glyph(font, glyph(font, character), pen, rect.y.into(), color);
                    }
                }
                Shape::Icon(icon) => {
                    let font = &ICONS[match rect.width { 16 => 0, 20 => 1, _ => 2 }];
                    canvas.glyph(font, &font.glyphs[icon as usize], rect.x.into(), rect.y.into(), color);
                }
            }
        }
    }

    fn character(key: u8, shift: bool, alpha: bool) -> Option<char> {
        let typed = CHARACTERS.get(usize::from(key))?;
        let typed = match (shift, alpha) {
            (true, true) => typed[2].to_ascii_uppercase(),
            (false, true) => typed[2],
            (true, false) => typed[1],
            (false, false) => typed[0],
        };
        (typed != 0).then_some(char::from(typed))
    }
}

pub mod color {
    const fn rgb(hex: u32) -> u16 {
        (hex >> 8 & 0xF800 | hex >> 5 & 0x07E0 | hex >> 3 & 0x001F) as u16
    }

    pub const BLACK: u16 = 0;
    pub const WHITE: u16 = rgb(0xFFFFFF);
    pub const NEUTRAL_100: u16 = rgb(0xF5F5F5);
    pub const NEUTRAL_200: u16 = rgb(0xE5E5E5);
    pub const NEUTRAL_400: u16 = rgb(0xA3A3A3);
    pub const NEUTRAL_500: u16 = rgb(0x737373);
    pub const NEUTRAL_600: u16 = rgb(0x525252);
    pub const NEUTRAL_700: u16 = rgb(0x404040);
    pub const NEUTRAL_800: u16 = rgb(0x262626);
    pub const NEUTRAL_900: u16 = rgb(0x171717);
    pub const SKY_100: u16 = rgb(0xE0F2FE);
    pub const SKY_300: u16 = rgb(0x7DD3FC);
    pub const SKY_400: u16 = rgb(0x38BDF8);
    pub const SKY_500: u16 = rgb(0x0EA5E9);
    pub const SKY_800: u16 = rgb(0x075985);
    pub const BLUE_500: u16 = rgb(0x3B82F6);
    pub const EMERALD_600: u16 = rgb(0x059669);
    pub const AMBER_600: u16 = rgb(0xD97706);
    pub const VIOLET_500: u16 = rgb(0x8B5CF6);
    pub const ROSE_500: u16 = rgb(0xF43F5E);
}
