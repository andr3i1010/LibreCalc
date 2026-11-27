// SPDX-License-Identifier: GPL-3.0-only
#![no_std]

extern crate alloc;

use alloc::string::String;
use graphite_ui::{Align, Icon, Ui, color::*};
use librecalc_abi::{Key, setting};

librecalc_abi::app!("Hello", main, graphite_ui::app_icon(Icon::Smile, ROSE_500));

#[derive(Default)]
struct State {
    slider: i16,
    name: String,
    dialog: bool,
    menu: bool,
    picked: &'static str,
}

const FRUITS: [&str; 3] = ["Apple", "Banana", "Cherry"];

fn main() -> u32 {
    let mut state = State {
        slider: 4,
        picked: "None",
        ..State::default()
    };
    graphite_ui::run(|ui| gallery(ui, &mut state))
}

fn gallery(ui: &mut Ui, state: &mut State) {
    if ui.pressed(Key::Back) {
        ui.exit(0);
    }
    let light = librecalc_abi::setting(setting::THEME) == Ok(1);
    ui.status_bar("Hello");
    ui.column()
        .scroll()
        .grow(1)
        .padding(10, 4)
        .gap(6)
        .show(|ui| {
            ui.toggle("Light theme", light).on_change(|light| {
                let _ = librecalc_abi::set_setting(setting::THEME, light.into());
            });
            ui.slider("Slider", state.slider, 10)
                .on_change(|value| state.slider = value);
            ui.progress(state.slider, 10);
            ui.text_field("Your name", &state.name)
                .on_change(|name| state.name = name);
            ui.list_row(Icon::Folder, "Fruit", state.picked)
                .on_click(|| state.menu = true);
            ui.button("Delete everything")
                .on_click(|| state.dialog = true);
            ui.row()
                .gap(10)
                .align(Align::Center)
                .padding(10, 4)
                .show(|ui| {
                    ui.spinner();
                    ui.text("Working on it");
                });
        });
    if state.dialog {
        let message = "This can't be undone.";
        ui.dialog("Delete everything?", message, &["Cancel", "Delete"])
            .on_change(|_| state.dialog = false);
    }
    if state.menu {
        ui.menu("Fruit", &FRUITS).on_change(|picked| {
            state.menu = false;
            if let Some(index) = picked {
                state.picked = FRUITS[index];
            }
        });
    }
}
