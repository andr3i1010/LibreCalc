// SPDX-License-Identifier: GPL-3.0-only

use crate::{Align, Font, Icon, Response, Theme, Ui};
use alloc::string::String;
use librecalc_abi::Key;

pub struct Change<T>(pub Option<T>);

impl<T> Change<T> {
    pub fn on_change(self, change: impl FnOnce(T)) {
        self.0.map(change);
    }
}

impl Response {
    pub fn on_click(self, click: impl FnOnce()) {
        if self.clicked {
            click();
        }
    }
}

impl Ui {
    pub fn status_bar(&mut self, title: &str) {
        let battery = alloc::format!("{}%", librecalc_abi::battery().unwrap_or(0) & 0xFF);
        let Theme {
            secondary, accent, ..
        } = self.theme;
        self.row()
            .height(22)
            .padding(12, 3)
            .gap(4)
            .align(Align::Center)
            .show(|ui| {
                ui.text(title).font(Font::SmallStrong).color(secondary);
                ui.spacer();
                let modifier = match (ui.shift, ui.alpha) {
                    (false, 0) => "",
                    (true, 0) => "shift",
                    (false, _) => "alpha",
                    (true, _) => "ALPHA",
                };
                if !modifier.is_empty() {
                    ui.text(modifier).font(Font::SmallStrong).color(accent);
                }
                if ui.alpha == 2 {
                    ui.icon(Icon::Lock).size(16).color(accent);
                }
                ui.text(&battery).font(Font::SmallStrong).color(secondary);
                ui.icon(Icon::BatteryFull).size(16).color(secondary);
            });
    }

    pub fn hints(&mut self, hints: &[(&str, &str)]) {
        let Theme {
            track, secondary, ..
        } = self.theme;
        self.row()
            .padding(12, 5)
            .gap(12)
            .align(Align::Center)
            .show(|ui| {
                for &(key, action) in hints {
                    ui.row()
                        .background(track)
                        .radius(4)
                        .padding(5, 1)
                        .show(|ui| {
                            ui.text(key).font(Font::SmallStrong);
                        });
                    ui.text(action).font(Font::Small).color(secondary);
                }
            });
    }

    pub fn button(&mut self, label: &str) -> Response {
        self.item(|ui| {
            ui.spacer();
            ui.text(label).font(Font::BodyStrong);
            ui.spacer();
        })
    }

    pub fn list_row(&mut self, icon: Icon, label: &str, value: &str) -> Response {
        let secondary = self.theme.secondary;
        self.item(|ui| {
            ui.icon(icon);
            ui.text(label);
            ui.spacer();
            ui.text(value).color(secondary);
            ui.icon(Icon::ChevronRight).size(16).color(secondary);
        })
    }

    pub fn toggle(&mut self, label: &str, on: bool) -> Change<bool> {
        let Theme { track, accent, .. } = self.theme;
        let response = self.item(|ui| {
            ui.text(label);
            ui.spacer();
            let color = if on { accent } else { track };
            ui.row()
                .width(34)
                .height(20)
                .padding(2, 2)
                .radius(10)
                .background(color)
                .show(|ui| {
                    let left = ui.animate(if on { 100 } else { 0 }, 150);
                    ui.column().grow(left);
                    ui.column()
                        .width(16)
                        .radius(8)
                        .background(crate::color::WHITE);
                    ui.column().grow(100 - left);
                });
        });
        Change(response.clicked.then_some(!on))
    }

    pub fn slider(&mut self, label: &str, value: i16, max: i16) -> Change<i16> {
        let Theme { track, accent, .. } = self.theme;
        let response = self.item(|ui| {
            ui.text(label);
            ui.spacer();
            ui.row().width(120).align(Align::Center).show(|ui| {
                ui.column()
                    .height(4)
                    .radius(2)
                    .background(accent)
                    .grow(value);
                ui.column()
                    .width(12)
                    .height(12)
                    .radius(6)
                    .background(accent);
                ui.column()
                    .height(4)
                    .radius(2)
                    .background(track)
                    .grow(max - value);
            });
        });
        let step = i16::from(response.focused && self.take(Key::Right))
            - i16::from(response.focused && self.take(Key::Left));
        let next = (value + step).clamp(0, max);
        Change((next != value).then_some(next))
    }

    pub fn progress(&mut self, value: i16, max: i16) {
        let Theme { track, accent, .. } = self.theme;
        self.row().height(6).radius(3).background(track).show(|ui| {
            ui.column().radius(3).background(accent).grow(value);
            ui.column().grow(max - value);
        });
    }

    pub fn divider(&mut self) {
        let track = self.theme.track;
        self.column().height(1).background(track);
    }

    pub fn text_field(&mut self, placeholder: &str, value: &str) -> Change<String> {
        let Theme {
            secondary, accent, ..
        } = self.theme;
        let focused = self.focusables == self.focus;
        let response = self.item(|ui| {
            ui.row().gap(1).align(Align::Center).show(|ui| {
                if !value.is_empty() {
                    ui.text(value);
                }
                if focused {
                    ui.column().width(1).height(16).background(accent);
                }
                if value.is_empty() {
                    ui.text(placeholder).color(secondary);
                }
            });
        });
        let mut next = String::from(value);
        match response.focused.then(|| self.typed()).flatten() {
            Some(typed) => next.push(typed),
            None if response.focused && self.take(Key::Backspace) => _ = next.pop(),
            None => return Change(None),
        }
        Change(Some(next))
    }

    pub fn dialog(&mut self, title: &str, message: &str, buttons: &[&str]) -> Change<usize> {
        let (secondary, mut picked) = (self.theme.secondary, None);
        let back = self.panel(Align::Center, |ui| {
            ui.text(title).font(Font::TitleStrong);
            ui.text(message).color(secondary);
            ui.grid(buttons.len() as i16).gap(8).show(|ui| {
                for (index, button) in buttons.iter().enumerate() {
                    ui.button(button).on_click(|| picked = Some(index));
                }
            });
        });
        Change(picked.or(back.then_some(0)))
    }

    pub fn menu(&mut self, title: &str, items: &[&str]) -> Change<Option<usize>> {
        let (secondary, mut picked) = (self.theme.secondary, None);
        let back = self.panel(Align::End, |ui| {
            ui.text(title).font(Font::SmallStrong).color(secondary);
            for (index, item) in items.iter().enumerate() {
                ui.item(|ui| _ = ui.text(item))
                    .on_click(|| picked = Some(Some(index)));
            }
        });
        Change(picked.or(back.then_some(None)))
    }
    fn panel(&mut self, place: Align, content: impl FnOnce(&mut Ui)) -> bool {
        let Theme { surface, track, .. } = self.theme;
        let mut back = false;
        let panel = self
            .overlay(place)
            .width(260)
            .background(surface)
            .radius(16);
        panel.padding(14, 12).gap(8).show(|ui| {
            ui.theme.surface = track;
            content(ui);
            back = ui.take(Key::Back);
        });
        self.theme.surface = surface;
        back
    }

    fn item(&mut self, content: impl FnOnce(&mut Ui)) -> Response {
        let surface = self.theme.surface;
        let row = self.row().focusable().background(surface).radius(10);
        row.padding(10, 8)
            .gap(10)
            .align(Align::Center)
            .show(content)
    }
}
