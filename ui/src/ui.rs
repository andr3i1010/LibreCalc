// SPDX-License-Identifier: GPL-3.0-only
use crate::{Command, Font, Icon, Shape, character, color::*, draw, text_size};
use alloc::{string::String, vec::Vec};
use librecalc_abi::{AppHeader, Error, Key, Rect, SCREEN_COPY, setting};

#[derive(Clone, Copy, Default)]
pub struct Theme {
    pub background: u16,
    pub surface: u16,
    pub track: u16,
    pub text: u16,
    pub secondary: u16,
    pub accent: u16,
    pub focus: u16,
    pub focus_alpha: u8,
    pub ring: u16,
    pub ring_alpha: u8,
    pub focus_text: u16,
}

const DARK: Theme = Theme {
    background: NEUTRAL_900,
    surface: NEUTRAL_800,
    track: NEUTRAL_700,
    text: NEUTRAL_100,
    secondary: NEUTRAL_400,
    accent: SKY_400,
    focus: SKY_400,
    focus_alpha: 38,
    ring: SKY_400,
    ring_alpha: 77,
    focus_text: SKY_100,
};

const LIGHT: Theme = Theme {
    background: WHITE,
    surface: NEUTRAL_100,
    track: NEUTRAL_200,
    text: NEUTRAL_900,
    secondary: NEUTRAL_600,
    accent: SKY_400,
    focus: SKY_500,
    focus_alpha: 26,
    ring: SKY_300,
    ring_alpha: 153,
    focus_text: SKY_800,
};

const SCREEN: Rect = Rect {
    x: 0,
    y: 0,
    width: 320,
    height: 240,
};

#[derive(Clone, Copy, Default)]
pub enum Align {
    Start,
    Center,
    End,
    #[default]
    Stretch,
}

#[derive(Default)]
enum Kind {
    #[default]
    Column,
    Row,
    Grid(i16),
    Text(String, Font),
    Icon(Icon, i16),
    Spinner,
}

#[derive(Default)]
struct Element {
    kind: Kind,
    gap: i16,
    padding: (i16, i16),
    align: Align,
    grow: i16,
    min: (i16, i16),
    scroll: bool,
    overlay: Option<Align>,
    background: Option<u16>,
    radius: u16,
    color: u16,
    focusable: Option<usize>,
    focused: bool,
    end: usize,
    size: Rect,
}

#[derive(Default)]
pub struct Ui {
    pub theme: Theme,
    elements: Vec<Element>,
    commands: Vec<Command>,
    targets: Vec<Rect>,
    modal: Option<core::ops::Range<usize>>,
    layer: usize,
    behind: Option<usize>,
    appear: Motion,
    scrolls: Vec<Motion>,
    scroll: usize,
    motions: Vec<Motion>,
    motion: usize,
    highlight: Motion,
    focused: Option<(Rect, u16, Rect)>,
    clip: Rect,
    now: u64,
    animating: bool,
    color: u16,
    pub(crate) focus: usize,
    pub(crate) focusables: usize,
    keys: u64,
    pub(crate) shift: bool,
    pub(crate) alpha: u8,
    exit: Option<u32>,
    entrance: Option<i16>,
    failed: Option<(u32, Error)>,
}

pub struct Response {
    pub focused: bool,
    pub clicked: bool,
}

#[derive(Clone, Copy, Default)]
struct Motion {
    key: i32,
    to: Rect,
    gap: Rect,
    start: u64,
    duration: u64,
}

impl Motion {
    fn shown(&self, now: u64) -> Rect {
        let left = 1.0 - ((now - self.start) as f32 / self.duration as f32).min(1.0);
        let left = left * left * left * left;
        zip(self.to, self.gap, |to, gap| {
            to + (f32::from(gap) * left) as i16
        })
    }

    fn at(
        &mut self,
        key: i32,
        to: Rect,
        now: u64,
        milliseconds: u64,
        animating: &mut bool,
    ) -> Rect {
        if key != self.key {
            let gap = zip(self.shown(now), to, |shown, to| shown - to);
            *self = Motion {
                key,
                gap,
                start: now,
                duration: milliseconds * 1000,
                ..*self
            };
        }
        self.to = to;
        *animating |= now < self.start + self.duration;
        self.shown(now)
    }

    fn value(&mut self, to: i16, now: u64, milliseconds: u64, animating: &mut bool) -> i16 {
        let rect = Rect {
            x: to,
            ..Rect::default()
        };
        self.at(to.into(), rect, now, milliseconds, animating).x
    }
}

fn zip(a: Rect, b: Rect, f: impl Fn(i16, i16) -> i16) -> Rect {
    Rect {
        x: f(a.x, b.x),
        y: f(a.y, b.y),
        width: f(a.width, b.width),
        height: f(a.height, b.height),
    }
}

pub fn run(mut app: impl FnMut(&mut Ui)) -> u32 {
    let mut ui = Ui {
        entrance: Some(320),
        ..Ui::default()
    };
    let mut previous = Vec::new();
    let frame = unsafe { core::slice::from_raw_parts_mut(SCREEN_COPY as *mut u16, 320 * 240) };
    let (mut held, mut repeat) = (0, 0u64);
    loop {
        ui.theme = if librecalc_abi::setting(setting::THEME) == Ok(1) {
            LIGHT
        } else {
            DARK
        };
        ui.now = librecalc_abi::uptime().unwrap_or(0);
        if ui.keys == 0 {
            let modifiers = librecalc_abi::modifiers().unwrap_or(0);
            (ui.shift, ui.alpha) = (modifiers & 1 != 0, (modifiers >> 1) as u8);
        }
        (
            ui.color,
            ui.focusables,
            ui.scroll,
            ui.motion,
            ui.clip,
            ui.animating,
        ) = (ui.theme.text, 0, 0, 0, SCREEN, false);
        ui.commands.clear();
        let background = ui.theme.background;
        ui.column().background(background).show(|ui| {
            app(ui);
            let Some((failed, error)) = ui.failed else {
                return;
            };
            let name = unsafe { &*(failed as *const AppHeader) }.name();
            let (what, why) = match error {
                Error::EFAULT => ("crashed", "It stopped working and was closed."),
                Error::EINTR => ("was force closed", "SHIFT, ALPHA and HOME close any app."),
                _ => ("couldn't open", "Too many apps open, or memory full."),
            };
            let title = alloc::format!("{name} {what}");
            if ui.dialog(&title, why, &["OK"]).0.is_some() {
                ui.failed = None;
            }
        });
        if let Some(code) = ui.exit {
            return code;
        }
        let overlays: Vec<usize> = (0..ui.elements.len())
            .filter(|&index| ui.elements[index].overlay.is_some())
            .collect();
        ui.measure(0);
        overlays.iter().for_each(|&overlay| ui.measure(overlay));
        ui.targets.resize(ui.focusables, Rect::default());
        ui.arrange(0, SCREEN);

        let to = if overlays.is_empty() { 0 } else { 256 };
        let shown = i32::from(ui.appear.value(to, ui.now, 180, &mut ui.animating));
        for overlay in overlays {
            let shape = Shape::Fill {
                radius: 0,
                alpha: (shown * 100 / 256) as u8,
            };
            ui.commands.push(Command {
                rect: SCREEN,
                clip: SCREEN,
                color: BLACK,
                shape,
            });
            let Rect { width, height, .. } = ui.elements[overlay].size;
            let rise = |distance: i16| (i32::from(distance) * (256 - shown) / 256) as i16;
            let rect = match ui.elements[overlay].overlay {
                Some(Align::End) => Rect {
                    x: 0,
                    y: 240 - height + rise(height),
                    width: 320,
                    height: height + 16,
                },
                _ => Rect {
                    x: (320 - width) / 2,
                    y: (240 - height) / 2 + rise(12),
                    width,
                    height,
                },
            };
            ui.arrange(overlay, rect);
        }
        ui.elements.clear();
        if let Some((rect, radius, clip)) = ui.focused.take() {
            let Theme {
                focus,
                focus_alpha,
                ring,
                ring_alpha,
                ..
            } = ui.theme;
            let rect = ui
                .highlight
                .at(ui.focus as i32, rect, ui.now, 150, &mut ui.animating);
            let shape = Shape::Fill {
                radius,
                alpha: focus_alpha,
            };
            ui.commands.push(Command {
                rect,
                clip,
                color: focus,
                shape,
            });
            let shape = Shape::Outline {
                radius,
                alpha: ring_alpha,
            };
            ui.commands.push(Command {
                rect,
                clip,
                color: ring,
                shape,
            });
        }

        let (focus, range) = match (ui.modal.take(), ui.behind) {
            (Some(range), None) => {
                ui.behind = Some(ui.focus);
                (range.start, range)
            }
            (Some(range), Some(_)) => (ui.focus, range),
            (None, Some(behind)) => (
                core::mem::take(&mut ui.behind).unwrap_or(behind),
                0..ui.targets.len(),
            ),
            (None, None) => (ui.focus, 0..ui.targets.len()),
        };
        let targets = &ui.targets[range.clone()];
        let focus = range.start + navigate(focus.saturating_sub(range.start), targets, ui.keys);
        if Key::Home.is_pressed(ui.keys) && librecalc_abi::pause().is_ok() {
            ui.entrance = Some(320);
        }
        if ui.keys != 0 || focus != ui.focus {
            (ui.focus, ui.keys) = (focus, 0);
            continue;
        }
        match ui.entrance.take() {
            Some(from) => {
                slide(&ui.commands, from, frame);
                held = librecalc_abi::keyboard_scan().unwrap_or(held);
            }
            None => render(&ui.commands, &previous, frame),
        }
        previous = core::mem::take(&mut ui.commands);

        ui.keys = next_keys(&mut held, &mut repeat, ui.animating);
    }
}

fn next_keys(held: &mut u64, repeat: &mut u64, animating: bool) -> u64 {
    const ARROWS: u64 = 0b1111;
    loop {
        let now = librecalc_abi::uptime().unwrap_or(0);
        let timeout = match *held & ARROWS {
            _ if animating => 0,
            0 => u32::MAX,
            _ => repeat.saturating_sub(now) as u32,
        };
        let keys = librecalc_abi::wait_event(*held, timeout).unwrap_or(*held);
        let now = librecalc_abi::uptime().unwrap_or(0);
        let mut pressed = keys & !*held;
        if pressed & ARROWS != 0 {
            *repeat = now + 400_000;
        } else if keys & ARROWS != 0 && now >= *repeat {
            (pressed, *repeat) = (keys & ARROWS, now + 80_000);
        }
        *held = keys;
        if pressed != 0 || animating {
            return pressed;
        }
    }
}

fn render(commands: &[Command], previous: &[Command], frame: &mut [u16]) {
    fn strip(commands: &[Command], top: i16) -> impl Iterator<Item = &Command> {
        commands.iter().filter(move |command| {
            command.rect.y < top + 30 && top < command.rect.y + command.rect.height
        })
    }
    let changed: Vec<i16> = (0..240)
        .step_by(30)
        .filter(|&top| !strip(commands, top).eq(strip(previous, top)))
        .collect();
    for &top in &changed {
        draw(
            commands,
            &mut frame[top as usize * 320..][..320 * 30],
            Rect {
                y: top,
                height: 30,
                ..SCREEN
            },
        );
    }
    if let (Some(&top), Some(&last)) = (changed.first(), changed.last()) {
        let rows = Rect {
            y: top,
            height: last + 30 - top,
            ..SCREEN
        };
        let _ = librecalc_abi::display_wait_for_vblank();
        let _ = librecalc_abi::display_push_rect(
            rows,
            &frame[top as usize * 320..(last as usize + 30) * 320],
        );
    }
}

fn slide(commands: &[Command], from: i16, frame: &mut [u16]) {
    draw(commands, frame, SCREEN);
    let to = Rect {
        x: from,
        ..Rect::default()
    };
    let mut motion = Motion {
        key: from.into(),
        to,
        ..Motion::default()
    };
    loop {
        let (now, mut moving) = (librecalc_abi::uptime().unwrap_or(0), false);
        let x = motion.value(0, now, 260, &mut moving);
        let (width, skip) = (320 - x.abs(), (-x).max(0) as usize);
        let _ = librecalc_abi::display_wait_for_vblank();
        for (y, row) in (0..).zip(frame.chunks(320)) {
            let rect = Rect {
                x: x.max(0),
                y,
                width,
                height: 1,
            };
            let _ = librecalc_abi::display_push_rect(rect, &row[skip..skip + width as usize]);
        }
        if !moving {
            break;
        }
    }
}

impl Ui {
    pub fn column(&mut self) -> Node<'_> {
        self.push(Kind::Column)
    }

    pub fn row(&mut self) -> Node<'_> {
        self.push(Kind::Row)
    }

    pub fn grid(&mut self, columns: i16) -> Node<'_> {
        self.push(Kind::Grid(columns.max(1)))
    }

    pub fn text(&mut self, text: &str) -> Node<'_> {
        self.push(Kind::Text(text.into(), Font::Body))
    }

    pub fn icon(&mut self, icon: Icon) -> Node<'_> {
        self.push(Kind::Icon(icon, 20))
    }

    pub fn spacer(&mut self) {
        self.column().grow(1);
    }

    pub fn spinner(&mut self) {
        self.push(Kind::Spinner);
    }

    pub fn overlay(&mut self, place: Align) -> Node<'_> {
        let mut node = self.column();
        node.element().overlay = Some(place);
        node
    }

    pub fn pressed(&self, key: Key) -> bool {
        key.is_pressed(self.keys) && (self.layer > 0 || self.behind.is_none())
    }

    pub fn take(&mut self, key: Key) -> bool {
        let pressed = self.pressed(key);
        if pressed {
            self.keys &= !(1 << key as u8);
        }
        pressed
    }

    pub fn typed(&mut self) -> Option<char> {
        let (shift, alpha) = (self.shift, self.alpha > 0);
        let (key, typed) = (0..64)
            .filter(|&key| self.keys >> key & 1 != 0)
            .find_map(|key| Some((key, character(key, shift, alpha)?)))
            .filter(|_| self.layer > 0 || self.behind.is_none())?;
        self.keys &= !(1 << key);
        Some(typed)
    }

    pub fn animate(&mut self, to: i16, milliseconds: u64) -> i16 {
        if self.motions.len() <= self.motion {
            self.motions.push(Motion {
                key: to.into(),
                ..Motion::default()
            });
        }
        self.motion += 1;
        self.motions[self.motion - 1].value(to, self.now, milliseconds, &mut self.animating)
    }

    pub fn set_focus(&mut self, focus: usize) {
        (self.focus, self.motion) = (focus, 0);
        self.scrolls.clear();
        self.motions.clear();
    }

    pub fn exit(&mut self, code: u32) {
        self.exit = Some(code);
    }

    pub fn launch(&mut self, app: u32) -> Result<u32, Error> {
        let result = librecalc_abi::launch(app);
        if matches!(result, Ok(_) | Err(Error::EFAULT | Error::EINTR)) {
            self.entrance = Some(-320);
        }
        self.failed = result.err().map(|error| (app, error));
        result
    }

    fn push(&mut self, kind: Kind) -> Node<'_> {
        let index = self.elements.len();
        self.elements.push(Element {
            kind,
            color: self.color,
            end: index + 1,
            ..Element::default()
        });
        Node { ui: self, index }
    }
}

pub struct Node<'a> {
    ui: &'a mut Ui,
    index: usize,
}

impl Node<'_> {
    fn element(&mut self) -> &mut Element {
        &mut self.ui.elements[self.index]
    }

    pub fn gap(mut self, gap: i16) -> Self {
        self.element().gap = gap;
        self
    }

    pub fn padding(mut self, x: i16, y: i16) -> Self {
        self.element().padding = (x, y);
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        self.element().align = align;
        self
    }

    pub fn grow(mut self, share: i16) -> Self {
        self.element().grow = share;
        self
    }

    pub fn width(mut self, width: i16) -> Self {
        self.element().min.0 = width;
        self
    }

    pub fn height(mut self, height: i16) -> Self {
        self.element().min.1 = height;
        self
    }

    pub fn scroll(mut self) -> Self {
        self.element().scroll = true;
        self
    }

    pub fn background(mut self, color: u16) -> Self {
        self.element().background = Some(color);
        self
    }

    pub fn radius(mut self, radius: u16) -> Self {
        self.element().radius = radius;
        self
    }

    pub fn color(mut self, color: u16) -> Self {
        self.element().color = color;
        self
    }

    pub fn font(mut self, font: Font) -> Self {
        if let Kind::Text(_, current) = &mut self.element().kind {
            *current = font;
        }
        self
    }

    pub fn size(mut self, size: i16) -> Self {
        if let Kind::Icon(_, current) = &mut self.element().kind {
            *current = size;
        }
        self
    }

    pub fn focusable(mut self) -> Self {
        let place = self.ui.focusables;
        self.ui.focusables += 1;
        let focused = place == self.ui.focus;
        let element = self.element();
        (element.focusable, element.focused) = (Some(place), focused);
        self
    }

    pub fn show(self, content: impl FnOnce(&mut Ui)) -> Response {
        let Node { ui, index } = self;
        let (focused, outer, first) = (ui.elements[index].focused, ui.color, ui.focusables);
        let overlay = ui.elements[index].overlay.is_some();
        if focused {
            ui.color = ui.theme.focus_text;
        }
        ui.layer += usize::from(overlay);
        content(ui);
        ui.layer -= usize::from(overlay);
        if overlay {
            ui.modal = Some(first..ui.focusables);
        }
        ui.color = outer;
        ui.elements[index].end = ui.elements.len();
        let clicked = focused && (ui.pressed(Key::OK) || ui.pressed(Key::EXE));
        Response { focused, clicked }
    }
}

fn turn(rect: Rect, column: bool) -> Rect {
    match column {
        true => Rect {
            x: rect.y,
            y: rect.x,
            width: rect.height,
            height: rect.width,
        },
        false => rect,
    }
}

impl Ui {
    fn children(&self, index: usize) -> Vec<usize> {
        let inside = |child: usize| (child < self.elements[index].end).then_some(child);
        core::iter::successors(inside(index + 1), |&child| inside(self.elements[child].end))
            .filter(|&child| self.elements[child].overlay.is_none())
            .collect()
    }

    fn measure(&mut self, index: usize) {
        let children = self.children(index);
        children.iter().for_each(|&child| self.measure(child));
        let element = &self.elements[index];
        let gaps = |count: usize| element.gap * (count as i16 - 1).max(0);
        let column = matches!(element.kind, Kind::Column);
        let sizes = children
            .iter()
            .map(|&child| turn(self.elements[child].size, column));
        let widest = sizes.clone().map(|size| size.width).max().unwrap_or(0);
        let tallest = sizes.clone().map(|size| size.height).max().unwrap_or(0);
        let size = match element.kind {
            Kind::Text(ref text, font) => text_size(text, font),
            Kind::Icon(_, size) => Rect {
                width: size,
                height: size,
                ..Rect::default()
            },
            Kind::Spinner => Rect {
                width: 20,
                height: 20,
                ..Rect::default()
            },
            Kind::Grid(columns) => {
                let rows = children.len().div_ceil(columns as usize);
                let width = columns * widest + gaps(columns as usize);
                Rect {
                    width,
                    height: rows as i16 * tallest + gaps(rows),
                    ..Rect::default()
                }
            }
            _ if element.scroll => turn(
                Rect {
                    height: tallest,
                    ..Rect::default()
                },
                column,
            ),
            _ => {
                let width = sizes.map(|size| size.width).sum::<i16>() + gaps(children.len());
                turn(
                    Rect {
                        width,
                        height: tallest,
                        ..Rect::default()
                    },
                    column,
                )
            }
        };
        let ((x, y), (width, height)) = (element.padding, element.min);
        self.elements[index].size = Rect {
            width: (size.width + 2 * x).max(width),
            height: (size.height + 2 * y).max(height),
            ..size
        };
    }

    fn arrange(&mut self, index: usize, rect: Rect) {
        let children = self.children(index);
        let clip = self.clip;
        let Element {
            ref kind,
            gap,
            padding: (x, y),
            align,
            scroll,
            background,
            radius,
            color,
            focusable,
            focused,
            ..
        } = self.elements[index];
        let mut draw = |rect, color, shape| {
            self.commands.push(Command {
                rect,
                clip,
                color,
                shape,
            })
        };
        if let Some(background) = background {
            draw(rect, background, Shape::Fill { radius, alpha: 255 });
        }
        if focused {
            self.focused = Some((rect, radius, clip));
        }
        match *kind {
            Kind::Text(ref text, font) => draw(rect, color, Shape::Text(text.clone(), font)),
            Kind::Icon(icon, size) => draw(
                Rect {
                    width: size,
                    height: size,
                    ..rect
                },
                color,
                Shape::Icon(icon),
            ),
            Kind::Spinner => {
                let phase = (self.now / 100_000) as i16;
                let dots = [
                    (7, 0),
                    (5, 5),
                    (0, 7),
                    (-5, 5),
                    (-7, 0),
                    (-5, -5),
                    (0, -7),
                    (5, -5),
                ];
                for (dot, (dx, dy)) in (0..).zip(dots) {
                    let (x, y) = (
                        rect.x + rect.width / 2 + dx - 2,
                        rect.y + rect.height / 2 + dy - 2,
                    );
                    let alpha = (255 - (phase - dot).rem_euclid(8) * 30) as u8;
                    draw(
                        Rect {
                            x,
                            y,
                            width: 4,
                            height: 4,
                        },
                        color,
                        Shape::Fill { radius: 2, alpha },
                    );
                }
                self.animating = true;
            }
            _ => {}
        }
        if let Some(place) = focusable {
            self.targets[place] = rect;
        }
        let content = Rect {
            x: rect.x + x,
            y: rect.y + y,
            width: rect.width - 2 * x,
            height: rect.height - 2 * y,
        };
        let column = matches!(kind, Kind::Column);
        if let Kind::Grid(columns) = *kind {
            let width = (content.width - (columns - 1) * gap) / columns;
            let height = children
                .iter()
                .map(|&child| self.elements[child].size.height)
                .max()
                .unwrap_or(0);
            for (cell, child) in (0..).zip(children) {
                let x = content.x + cell % columns * (width + gap);
                let y = content.y + cell / columns * (height + gap);
                self.arrange(
                    child,
                    Rect {
                        x,
                        y,
                        width,
                        height,
                    },
                );
            }
            return;
        }

        let slot = self.scroll;
        if scroll {
            self.scroll += 1;
            if self.scrolls.len() <= slot {
                self.scrolls.push(Motion::default());
            }
            self.clip = content;
        }
        let content = turn(content, column);
        let sizes: Vec<_> = children
            .iter()
            .map(|&child| turn(self.elements[child].size, column))
            .collect();
        let used = sizes.iter().map(|size| size.width).sum::<i16>()
            + gap * (sizes.len() as i16 - 1).max(0);
        let shares: i16 = children
            .iter()
            .map(|&child| self.elements[child].grow)
            .sum();
        let extra = (content.width - used).max(0);
        let commands = self.commands.len();
        let outside = if scroll { self.focused.take() } else { None };
        let mut offset = if scroll {
            self.scrolls[slot].shown(self.now).x
        } else {
            0
        };
        loop {
            let mut x = content.x - offset;
            for (&child, size) in children.iter().zip(&sizes) {
                let share = self.elements[child].grow;
                let width = size.width
                    + if shares > 0 {
                        (i32::from(extra) * i32::from(share) / i32::from(shares)) as i16
                    } else {
                        0
                    };
                let free = content.height - size.height;
                let (y, height) = match align {
                    Align::Start => (content.y, size.height),
                    Align::Center => (content.y + free / 2, size.height),
                    Align::End => (content.y + free, size.height),
                    Align::Stretch => (content.y, content.height),
                };
                self.arrange(
                    child,
                    turn(
                        Rect {
                            x,
                            y,
                            width,
                            height,
                        },
                        column,
                    ),
                );
                x += width + gap;
            }
            if !scroll {
                return;
            }
            let logical = self.scrolls[slot].to.x;
            let mut wanted = logical;
            if let Some((target, ..)) = self.focused {
                let target = turn(target, column);
                let x = target.x + offset - logical;
                let (start, end) = (x - content.x, x + target.width - content.x - content.width);
                wanted += if start < 0 {
                    start
                } else {
                    end.clamp(0, start)
                };
            }
            let wanted = wanted.clamp(0, (used - content.width).max(0));
            let shown = self.scrolls[slot].value(wanted, self.now, 220, &mut self.animating);
            if shown == offset {
                break;
            }
            offset = shown;
            self.commands.truncate(commands);
            self.focused = None;
            self.scroll = slot + 1;
        }
        (self.clip, self.focused) = (clip, self.focused.or(outside));
        if used > content.width {
            let outer = turn(rect, column);
            let length =
                (i32::from(content.width) * i32::from(content.width) / i32::from(used)) as i16;
            let start =
                content.x + (i32::from(offset) * i32::from(content.width) / i32::from(used)) as i16;
            let bar = Rect {
                x: start,
                y: outer.y + outer.height - 3,
                width: length,
                height: 2,
            };
            let shape = Shape::Fill {
                radius: 1,
                alpha: 255,
            };
            self.commands.push(Command {
                rect: turn(bar, column),
                clip,
                color: NEUTRAL_600,
                shape,
            });
        }
    }
}

fn navigate(focus: usize, targets: &[Rect], keys: u64) -> usize {
    let focus = focus.min(targets.len().saturating_sub(1));
    let center = |rect: Rect| {
        (
            i32::from(rect.x + rect.width / 2),
            i32::from(rect.y + rect.height / 2),
        )
    };
    let arrows = [
        (Key::Left, -1, 0),
        (Key::Right, 1, 0),
        (Key::Up, 0, -1),
        (Key::Down, 0, 1),
    ];
    let (Some(&from), Some((_, dx, dy))) = (
        targets.get(focus),
        arrows.into_iter().find(|(key, ..)| key.is_pressed(keys)),
    ) else {
        return focus;
    };
    let (x, y) = center(from);
    let nearest = (0..).zip(targets).filter_map(|(index, &target)| {
        let (tx, ty) = center(target);
        let (main, cross) = (
            (tx - x) * dx + (ty - y) * dy,
            ((tx - x) * dy + (ty - y) * dx).abs(),
        );
        (main > 0).then_some((main + 2 * cross, index))
    });
    let next = focus.wrapping_add_signed(dx as isize);
    nearest
        .min()
        .map(|(_, index)| index)
        .or((dx != 0 && next < targets.len()).then_some(next))
        .unwrap_or(focus)
}
