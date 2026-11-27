// SPDX-License-Identifier: GPL-3.0-only
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use graphite_ui::{Align, Font, Icon, color::*};
use librecalc_abi::AppHeader;

librecalc_abi::app!("Home", main, 0);

fn main() -> u32 {
    let mut headers = [0; 64];
    let count = librecalc_abi::apps(&mut headers).unwrap_or(0) as usize;
    unsafe extern "C" {
        static __app_header: AppHeader;
    }
    let apps: Vec<&AppHeader> = headers[..count.min(64)]
        .iter()
        .map(|&address| unsafe { &*(address as *const AppHeader) })
        .filter(|&app| !core::ptr::eq(app, &raw const __app_header))
        .collect();
    graphite_ui::run(|ui| {
        let surface = ui.theme.surface;
        ui.status_bar("LibreCalc");
        ui.column().scroll().grow(1).padding(10, 4).show(|ui| {
            if apps.is_empty() {
                ui.text("No apps installed??");
            }
            ui.grid(3).gap(8).show(|ui| {
                for app in &apps {
                    let icon = app.icon();
                    let (color, icon) =
                        ((icon >> 16) as u16, Icon::ALL.get(icon as usize & 0xFFFF));
                    let tile = ui.column().focusable().background(surface).radius(14);
                    let response = tile.padding(0, 12).gap(8).align(Align::Center).show(|ui| {
                        ui.column()
                            .background(color)
                            .radius(12)
                            .padding(8, 8)
                            .show(|ui| {
                                ui.icon(*icon.unwrap_or(&Icon::CircleDashed))
                                    .size(24)
                                    .color(WHITE);
                            });
                        ui.text(app.name()).font(Font::BodyStrong);
                    });
                    if response.clicked {
                        let _ = ui.launch(app.address());
                    }
                }
            });
        });
    })
}
