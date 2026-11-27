// SPDX-License-Identifier: GPL-3.0-only
use std::{fmt::Write, fs};

const FONTS: [(&str, &str, f32); 6] = [
    ("Small", "Medium", 11.0),
    ("SmallStrong", "SemiBold", 11.0),
    ("Body", "Medium", 13.0),
    ("BodyStrong", "SemiBold", 13.0),
    ("Title", "Medium", 15.0),
    ("TitleStrong", "SemiBold", 15.0),
];
const ICON_SIZES: [f32; 3] = [16.0, 20.0, 24.0];
const ICONS: [(&str, char); 20] = [
    ("BatteryFull", '\u{e055}'),
    ("Calculator", '\u{e1bc}'),
    ("ChartColumn", '\u{e2a3}'),
    ("ChartSpline", '\u{e60d}'),
    ("Check", '\u{e06c}'),
    ("ChevronLeft", '\u{e06e}'),
    ("ChevronRight", '\u{e06f}'),
    ("CircleDashed", '\u{e4b0}'),
    ("Code", '\u{e093}'),
    ("Copy", '\u{e09e}'),
    ("Folder", '\u{e0d7}'),
    ("Gamepad2", '\u{e0df}'),
    ("LoaderCircle", '\u{e10a}'),
    ("Pencil", '\u{e1f9}'),
    ("Play", '\u{e13c}'),
    ("Settings", '\u{e154}'),
    ("Sigma", '\u{e201}'),
    ("Smile", '\u{e164}'),
    ("Trash2", '\u{e18e}'),
    ("Lock", '\u{e10b}'),
];

fn rasterize(path: &str, size: f32, characters: impl Iterator<Item = char>) -> String {
    println!("cargo:rerun-if-changed={path}");
    let font = fontdue::Font::from_bytes(fs::read(path).unwrap(), Default::default()).unwrap();
    let line = font.horizontal_line_metrics(size).unwrap();
    let (mut glyphs, mut bits) = (String::new(), Vec::new());
    for character in characters {
        let (metrics, coverage) = font.rasterize(character, size);
        let top = line.ascent.round() as i32 - metrics.ymin - metrics.height as i32;
        write!(
            glyphs,
            "Glyph {{ width: {}, height: {}, left: {}, top: {top}, advance: {}, start: {} }}, ",
            metrics.width,
            metrics.height,
            metrics.xmin,
            metrics.advance_width.round(),
            bits.len()
        )
        .unwrap();
        let nibbles: Vec<u8> = coverage
            .iter()
            .map(|&value| (u16::from(value) * 15 / 255) as u8)
            .collect();
        bits.extend(
            nibbles
                .chunks(2)
                .map(|pair| pair[0] | pair.get(1).unwrap_or(&0) << 4),
        );
    }
    format!(
        "    FontData {{ line_height: {}, glyphs: &[{glyphs}], bits: &{bits:?} }},\n",
        line.new_line_size.round()
    )
}

fn main() {
    let mut out = String::new();
    let names: Vec<&str> = FONTS.iter().map(|font| font.0).collect();
    writeln!(
        out,
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub enum Font {{ {} }}",
        names.join(", ")
    )
    .unwrap();
    out += "static FONTS: [FontData; 6] = [\n";
    for (_, weight, size) in FONTS {
        let path = format!("assets/PlusJakartaSans-{weight}.ttf");
        out += &rasterize(
            &path,
            size,
            (0x20..=0xFF).map(|code| char::from_u32(code).unwrap()),
        );
    }
    out += "];\n";

    let names: Vec<&str> = ICONS.iter().map(|icon| icon.0).collect();
    writeln!(
        out,
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub enum Icon {{ {} }}",
        names.join(", ")
    )
    .unwrap();
    writeln!(
        out,
        "impl Icon {{ pub const ALL: [Icon; {}] = [Icon::{}]; }}",
        names.len(),
        names.join(", Icon::")
    )
    .unwrap();
    out += "static ICONS: [FontData; 3] = [\n";
    for size in ICON_SIZES {
        out += &rasterize("assets/lucide.ttf", size, ICONS.iter().map(|icon| icon.1));
    }
    out += "];\n";

    fs::write(
        format!("{}/assets.rs", std::env::var("OUT_DIR").unwrap()),
        out,
    )
    .unwrap();
}
