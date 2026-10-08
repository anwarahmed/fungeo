//! All drawing: the layout of each screen, the river scene, the flags, and the list of
//! clickable rectangles (`App::buttons`), which is rebuilt every time.
//!
//! Pictures are drawn on a `Canvas` of square pixels, two to a cell (`▀` with a color
//! above and a color below). Words a child has to read are drawn in the big letters of
//! `font` wherever they fit, and as ordinary bold text where they do not.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::app::{Action, App, DROP, INTRO_LETTER, Phase, SINK, Screen};
use crate::font;
use crate::quiz::{Asked, Flag, Level, PLANKS};
use crate::theme::{Rgb, Theme};

/// The window has to be at least this big.
pub const MIN: (u16, u16) = (60, 22);

fn color(c: Rgb) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

/// `a` with `t` (from 0 to 1) of `b` mixed in.
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let part = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t.clamp(0.0, 1.0)).round() as u8;
    (part(a.0, b.0), part(a.1, b.1), part(a.2, b.2))
}

/// Dark or white, whichever can be read on `bg`.
fn ink(bg: Rgb) -> Rgb {
    let light = 0.299 * bg.0 as f32 + 0.587 * bg.1 as f32 + 0.114 * bg.2 as f32;
    if light > 150.0 { (33, 28, 38) } else { (255, 255, 255) }
}

fn fill(buf: &mut Buffer, r: Rect, bg: Rgb) {
    let r = r.intersection(buf.area);
    for y in r.top()..r.bottom() {
        for x in r.left()..r.right() {
            buf[(x, y)].reset();
            buf[(x, y)].set_bg(color(bg));
        }
    }
}

/// One line of ordinary text, bold, on whatever background is there. Cut at `max` cells.
fn put(buf: &mut Buffer, x: u16, y: u16, text: &str, fg: Rgb, max: u16) {
    if x < buf.area.right() && y < buf.area.bottom() {
        buf.set_stringn(x, y, text, max as usize, Style::new().fg(color(fg)).add_modifier(Modifier::BOLD));
    }
}

/// The same, in the middle of `r`'s width, on row `y`.
fn centered(buf: &mut Buffer, r: Rect, y: u16, text: &str, fg: Rgb) {
    let width = (text.chars().count() as u16).min(r.width);
    put(buf, r.x + (r.width - width) / 2, y, text, fg, width);
}

/// Breaks ordinary text into lines of at most `width` characters.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

/// The lines `text` makes in big letters `scale` times the font's size, if it fits `r`.
fn big_lines(text: &str, r: Rect, scale: usize) -> Option<Vec<String>> {
    let lines = font::wrap(text, r.width as usize / scale)?;
    (!lines.is_empty() && lines.len() as u16 * line_rows(scale) <= r.height).then_some(lines)
}

/// Rows of cells one line of big letters takes, with the space under it.
fn line_rows(scale: usize) -> u16 {
    font::rows(scale) + u16::from(scale > 1)
}

/// Writes `text` in the middle of `r`: in big letters up to `scale` times the font's
/// size where that fits (`scale` 0 for never), and otherwise as ordinary bold text.
fn label(buf: &mut Buffer, r: Rect, text: &str, fg: Rgb, scale: usize) {
    if r.width == 0 || r.height == 0 {
        return;
    }
    for scale in (1..=scale).rev() {
        if let Some(lines) = big_lines(text, r, scale) {
            // In pixels: the last row of a line is half a cell, so an odd number of
            // rows still looks centered.
            let tall = lines.len() as i32 * line_rows(scale) as i32 * 2 - if scale == 1 { 1 } else { 2 };
            let top = r.y as i32 * 2 + (r.height as i32 * 2 - tall).max(0) / 2;
            for (i, line) in lines.iter().enumerate() {
                let wide = (font::width(line) * scale) as i32;
                let x = r.x as i32 + (r.width as i32 - wide) / 2;
                font::draw(buf, x, top + i as i32 * line_rows(scale) as i32 * 2, line, color(fg), scale);
            }
            return;
        }
    }
    let lines = wrap(text, r.width as usize);
    let shown = lines.len().min(r.height as usize);
    let top = r.y + (r.height - shown as u16) / 2;
    for (i, line) in lines.iter().take(shown).enumerate() {
        centered(buf, r, top + i as u16, line, fg);
    }
}

/// A line around the inside edge of `r`.
fn frame(buf: &mut Buffer, r: Rect, fg: Rgb) {
    let (right, bottom) = (r.right() - 1, r.bottom() - 1);
    for x in r.x + 1..right {
        put(buf, x, r.y, "━", fg, 1);
        put(buf, x, bottom, "━", fg, 1);
    }
    for y in r.y + 1..bottom {
        put(buf, r.x, y, "┃", fg, 1);
        put(buf, right, y, "┃", fg, 1);
    }
    for (x, y, corner) in [(r.x, r.y, "┏"), (right, r.y, "┓"), (r.x, bottom, "┗"), (right, bottom, "┛")] {
        put(buf, x, y, corner, fg, 1);
    }
}

/// A filled, clickable rectangle. With `marked`, it has a frame (or arrows at its
/// ends when it is too flat for one) to show that Enter presses it. Returns the part
/// inside the frame, where its words go.
fn button(buf: &mut Buffer, app: &mut App, r: Rect, action: Option<Action>, bg: Rgb, marked: bool) -> Rect {
    fill(buf, r, bg);
    if let Some(action) = action {
        app.buttons.push((r, action));
    }
    let fg = ink(bg);
    if r.height >= 3 && r.width >= 4 {
        if marked {
            frame(buf, r, fg);
        }
        Rect::new(r.x + 2, r.y + 1, r.width - 4, r.height - 2)
    } else {
        if marked && r.width >= 4 {
            for y in r.top()..r.bottom() {
                put(buf, r.x, y, "▶", fg, 1);
                put(buf, r.right() - 1, y, "◀", fg, 1);
            }
        }
        Rect::new(r.x + 1.min(r.width), r.y, r.width.saturating_sub(2), r.height)
    }
}

/// Square pixels, two to a cell.
struct Canvas {
    w: i32,
    h: i32,
    px: Vec<Rgb>,
}

impl Canvas {
    /// As many pixels as `cols` by `rows` cells hold.
    fn new(cols: u16, rows: u16, fill: Rgb) -> Canvas {
        let (w, h) = (cols as i32, rows as i32 * 2);
        Canvas { w, h, px: vec![fill; (w * h) as usize] }
    }

    fn set(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.px[(y * self.w + x) as usize] = c;
        }
    }

    fn get(&self, x: i32, y: i32) -> Rgb {
        self.px[(y.clamp(0, self.h - 1) * self.w + x.clamp(0, self.w - 1)) as usize]
    }

    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        for py in y..y + h {
            for px in x..x + w {
                self.set(px, py, c);
            }
        }
    }

    /// A filled circle.
    fn disc(&mut self, cx: f32, cy: f32, r: f32, c: Rgb) {
        for y in (cy - r).floor() as i32..=(cy + r).ceil() as i32 {
            for x in (cx - r).floor() as i32..=(cx + r).ceil() as i32 {
                if (x as f32 - cx).powi(2) + (y as f32 - cy).powi(2) <= r * r {
                    self.set(x, y, c);
                }
            }
        }
    }

    /// Rows of a picture, each character a color of `palette` or nothing, with each of
    /// its pixels `scale` pixels across.
    fn sprite(&mut self, x: i32, y: i32, rows: &[&str], palette: &[(u8, Rgb)], scale: i32) {
        for (j, row) in rows.iter().enumerate() {
            for (i, key) in row.bytes().enumerate() {
                if let Some(&(_, c)) = palette.iter().find(|(k, _)| *k == key) {
                    self.rect(x + i as i32 * scale, y + j as i32 * scale, scale, scale, c);
                }
            }
        }
    }

    /// Puts the canvas on the screen with its top left corner at `at`.
    fn blit(&self, buf: &mut Buffer, at: (u16, u16)) {
        for row in 0..self.h / 2 {
            for x in 0..self.w {
                if let Some(cell) = buf.cell_mut((at.0 + x as u16, at.1 + row as u16)) {
                    cell.reset();
                    cell.set_symbol("▀").set_fg(color(self.get(x, row * 2))).set_bg(color(self.get(x, row * 2 + 1)));
                }
            }
        }
    }
}

/// What the river scene shows at one moment.
struct Scene {
    /// Planks lying on the bridge.
    laid: usize,
    /// A plank in the air: its place on the bridge, whether it will land there, and
    /// how many seconds it has been falling.
    drop: Option<(usize, bool, f32)>,
    /// Where the explorer is, in planks from the near bank.
    walker: f32,
    /// The explorer has arrived and is jumping for joy.
    cheering: bool,
    /// The flag on the far bank.
    flag: Rgb,
    time: f32,
}

const EXPLORER: [[&str; 8]; 2] = [
    [".HHHHH.", "HHHHHHH", ".SESES.", ".SSSSS.", "KBBBBB.", "KBBBBBS", ".PPPPP.", ".P...P."],
    [".HHHHH.", "HHHHHHH", ".SESES.", ".SSSSS.", "KBBBBB.", "KBBBBBS", ".PPPPP.", "..P.P.."],
];

fn scene(buf: &mut Buffer, r: Rect, theme: &Theme, s: &Scene) {
    if r.width < 20 || r.height < 3 {
        return;
    }
    let mut c = Canvas::new(r.width, r.height, theme.sky[0]);
    let (w, h) = (c.w, c.h);
    // Everything is twice the size in a tall scene, so that it does not look lost.
    let unit = if h >= 24 { 2 } else { 1 };
    // The top of the banks and of the bridge.
    let deck = h - 3 * unit - 2;
    let bank = (w * 16 / 100).max(9 * unit);
    let gap = w - 2 * bank;
    let plank = gap as f32 / PLANKS as f32;

    for y in 0..deck {
        let sky = mix(theme.sky[0], theme.sky[1], y as f32 / deck.max(1) as f32);
        c.rect(0, y, w, 1, sky);
    }
    if theme.night {
        // Stars that twinkle, always in the same places.
        for i in 0..w / 3 {
            let (x, y) = ((i * i * 31 + i * 17 + 11) % w, (i * i * 7 + i * 13 + 5) % (deck - 4).max(1));
            let bright = 0.5 + 0.5 * (s.time * 2.0 + i as f32).sin();
            c.set(x, y, mix(c.get(x, y), (255, 255, 255), 0.25 + 0.6 * bright));
        }
    }
    c.disc(w as f32 * 0.8, 2.5 * unit as f32, 2.4 * unit as f32, theme.sun);
    if !theme.night {
        for (i, (start, height, speed)) in [(0.1, 1, 1.6), (0.45, 3, 1.0), (0.7, 0, 2.2)].into_iter().enumerate() {
            let x = ((start * w as f32 + s.time * speed + i as f32 * 7.0) as i32).rem_euclid(w + 24) - 12;
            let (y, cloud) = (height * unit + 1, mix(theme.sky[1], (255, 255, 255), 0.85));
            c.rect(x + 2 * unit, y, 5 * unit, unit, cloud);
            c.rect(x, y + unit, 10 * unit, unit, cloud);
        }
    }
    // Hills on the far side of the river.
    let far = mix(theme.grass, theme.sky[1], 0.55);
    for x in 0..w {
        let top = deck as f32 - (2.5 + 1.5 * (x as f32 * 0.09 / unit as f32).sin() + (x as f32 * 0.031 / unit as f32 + 2.0).sin()) * unit as f32;
        c.rect(x, top as i32, 1, deck - top as i32, far);
    }

    // The river, with light running along it, and its two banks.
    let surface = deck + 2 * unit;
    c.rect(0, deck, w, h - deck, theme.water[0]);
    for y in surface..h {
        for x in bank..bank + gap {
            let wave = (x as f32 / unit as f32 + s.time * 5.0 + (y / unit) as f32 * 4.0) as i32;
            if (y / unit) % 2 == 0 && wave.rem_euclid(9) < 3 {
                c.set(x, y, theme.water[1]);
            }
        }
    }
    c.rect(bank, deck, gap, surface - deck, mix(theme.water[0], theme.sky[1], 0.35));
    let earth = mix(theme.grass, (70, 45, 20), 0.55);
    for x in [0, bank + gap] {
        c.rect(x, deck, bank, h - deck, earth);
        c.rect(x, deck, bank, unit, theme.grass);
    }

    // A tree on the near bank, and the flag to reach on the far one.
    let leaves = mix(theme.grass, (0, 60, 20), 0.35);
    c.rect(2 * unit, deck - 3 * unit, unit, 3 * unit, theme.wood);
    c.disc(2.5 * unit as f32, deck as f32 - 4.5 * unit as f32, 2.2 * unit as f32, leaves);
    let pole = w - 3 * unit;
    c.rect(pole, deck - 8 * unit, unit, 8 * unit, ink(theme.sky[1]));
    for i in 0..4 * unit {
        // The flag waves: each column rides a little higher or lower than the last.
        let lift = ((s.time * 6.0 - i as f32 / unit as f32 * 1.3).sin() * 0.8 * unit as f32).round() as i32;
        c.rect(pole - 1 - i, deck - 8 * unit + lift.max(0), 1, 3 * unit, s.flag);
    }

    // The bridge: posts at both ends, the planks laid so far, and marks where the
    // others will go.
    let (wood, dark) = (theme.wood, mix(theme.wood, (0, 0, 0), 0.35));
    for x in [bank - unit, bank + gap] {
        c.rect(x, deck - 2 * unit, unit, 3 * unit + 1, dark);
    }
    let slot = |i: usize| (bank + (i as f32 * plank).round() as i32, bank + ((i + 1) as f32 * plank).round() as i32);
    for i in 0..PLANKS {
        let (from, to) = slot(i);
        if i < s.laid {
            c.rect(from, deck, to - from, unit, if i % 2 == 0 { wood } else { mix(wood, (255, 255, 255), 0.12) });
            c.rect(from, deck + unit, to - from, (unit + 1) / 2, dark);
            c.rect(to - 1, deck, 1, unit, dark);
        } else {
            for x in (from + 1..to - 1).step_by(2) {
                c.set(x, deck, mix(wood, theme.sky[1], 0.45));
            }
        }
    }
    if let Some((place, good, age)) = s.drop {
        let (from, to) = slot(place.min(PLANKS - 1));
        if good {
            // Faster and faster, until it lands.
            let fallen = (age / DROP).clamp(0.0, 1.0).powi(2);
            let y = (-2.0 + fallen * (deck as f32 + 2.0)) as i32;
            c.rect(from, y.min(deck), to - from, unit, wood);
            c.rect(from, y.min(deck) + unit, to - from, (unit + 1) / 2, dark);
        } else {
            // It misses, tips into the river, and leaves a splash and rings.
            let fall = SINK * 0.5;
            let fallen = (age / fall).clamp(0.0, 1.0).powi(2);
            let y = (-2.0 + fallen * (surface as f32 + 3.0)) as i32;
            let middle = (from + to) / 2;
            if age < fall {
                c.rect(from + 1, y, to - from - 1, unit, wood);
            } else {
                let after = (age - fall) / (SINK - fall);
                let foam = mix((255, 255, 255), theme.water[1], after);
                let reach = (after * plank * 0.9) as i32 + 1;
                for side in [-1, 1] {
                    c.rect(middle + side * reach, surface + unit, unit, unit, foam);
                    let up = ((1.0 - (after * 2.0 - 1.0).powi(2)) * 3.0 * unit as f32) as i32;
                    c.rect(middle + side * (reach / 2 + 1), surface - up, unit, unit, foam);
                }
            }
        }
    }

    // The explorer: on the near bank, on the newest plank, or on the far bank.
    let palette = [(b'H', theme.accent), (b'S', (242, 201, 160)), (b'E', (40, 30, 30)), (b'B', theme.answers[1]), (b'K', dark), (b'P', (50, 60, 110))];
    let x = bank as f32 + (s.walker - 0.5) * plank - 3.5 * unit as f32;
    let x = x.min((w - 12 * unit) as f32).max((bank - 8 * unit) as f32).max(0.0);
    let moving = s.walker.fract() != 0.0 && !s.cheering;
    let hop = if s.cheering {
        (s.time * 7.0).sin().abs() * 3.0
    } else if moving {
        (s.walker * std::f32::consts::PI * 2.0).sin().abs()
    } else {
        0.0
    };
    let frame = usize::from(moving && (s.walker * 4.0) as i32 % 2 == 0);
    c.sprite(x.round() as i32, deck - 8 * unit - (hop * unit as f32) as i32, &EXPLORER[frame], &palette, unit);

    c.blit(buf, (r.x, r.y));
}

/// A flag `rows` cells tall, in the middle of `r`, with a thin frame so that a white
/// flag shows on a pale background.
fn flag(buf: &mut Buffer, r: Rect, flag: &Flag, theme: &Theme) {
    let rows = r.height;
    let cols = (if flag.square() { rows * 2 } else { rows * 3 }).min(r.width);
    if rows < 2 || cols < 6 {
        return;
    }
    let mut c = Canvas::new(cols, rows, theme.dim);
    let (w, h) = (c.w - 2, c.h - 2);
    for y in 0..h {
        for x in 0..w {
            c.set(x + 1, y + 1, flag.at((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32));
        }
    }
    c.blit(buf, (r.x + (r.width - cols) / 2, r.y));
}

/// A row of three stars, the earned ones bright.
fn stars(buf: &mut Buffer, r: Rect, earned: u8, bright: Rgb, faint: Rgb, big: bool) {
    if big && r.height >= font::ROWS && r.width >= 23 {
        let x = r.x as i32 + (r.width as i32 - 23) / 2;
        let y = r.y as i32 * 2 + (r.height as i32 * 2 - 7) / 2;
        for i in 0..3 {
            font::draw(buf, x + i * 9, y, "*", color(if (i as u8) < earned { bright } else { faint }), 1);
        }
    } else if r.width >= 5 && r.height >= 1 {
        let (x, y) = (r.x + (r.width - 5) / 2, r.y + r.height / 2);
        for i in 0..3u16 {
            let lit = (i as u8) < earned;
            put(buf, x + i * 2, y, if lit { "★" } else { "☆" }, if lit { bright } else { faint }, 1);
        }
    }
}

/// The title, each letter its own color, riding a wave. `landed` says how far each
/// letter has still to fall (in pixels) during the intro.
fn title(buf: &mut Buffer, r: Rect, theme: &Theme, scale: usize, time: f32, falling: impl Fn(usize) -> Option<f32>) {
    let name = "FUNGEO";
    let advance = (6 * scale + scale) as i32;
    let wide = advance * name.len() as i32 - 2 * scale as i32;
    let x = r.x as i32 + (r.width as i32 - wide) / 2;
    let colors = [theme.answers[0], theme.answers[2], theme.answers[3], theme.answers[1], theme.accent, theme.answers[0]];
    for (i, letter) in name.chars().enumerate() {
        let Some(above) = falling(i) else { continue };
        let wave = if above == 0.0 { ((time * 3.0 + i as f32 * 0.8).sin() * scale as f32).round() as i32 } else { 0 };
        let y = r.y as i32 * 2 + 2 * scale as i32 + wave - above as i32;
        // A shadow first, a pixel down and to the right, to lift the letter off the page.
        font::draw(buf, x + i as i32 * advance + 1, y + 1, &letter.to_string(), color(mix(theme.bg, (0, 0, 0), 0.25)), scale);
        font::draw(buf, x + i as i32 * advance, y, &letter.to_string(), color(colors[i]), scale);
    }
}

/// Rows the title needs at this scale: the letters, and room to ride the wave.
fn title_rows(scale: usize) -> u16 {
    font::rows(scale) + 2 * scale as u16
}

fn intro(buf: &mut Buffer, app: &mut App, area: Rect) {
    let theme = app.theme();
    let t = app.phase_time;
    app.buttons.push((area, Action::Skip));
    let scale = if area.width >= 100 && area.height >= 34 { 2 } else { 1 };
    let rows = title_rows(scale);
    let scene_rows = (area.height / 3).clamp(7, 14);
    let top = (area.height.saturating_sub(rows + scene_rows + 6)) / 2 + 1;
    let fall = |i: usize| {
        let since = t - i as f32 * INTRO_LETTER;
        if since < 0.0 {
            return None;
        }
        // Down from above the window, then one small bounce.
        let high = (top as f32 + rows as f32) * 2.0;
        let done = since / INTRO_LETTER;
        Some(if done < 1.0 { high * (1.0 - done * done) } else { (((done - 1.0) * 2.2).min(1.0) * std::f32::consts::PI).sin() * 3.0 * scale as f32 })
    };
    title(buf, Rect::new(area.x, top, area.width, rows), theme, scale, app.time, fall);

    // A plank for every letter, and then the explorer walks across.
    let landed = ((t / INTRO_LETTER) as usize).saturating_sub(1).min(6);
    let laid = (landed * PLANKS).div_ceil(6);
    let walk_from = INTRO_LETTER * 7.8;
    let walker = ((t - walk_from).max(0.0) * 4.2).min(PLANKS as f32 + 1.5);
    let cheering = walker >= PLANKS as f32 + 1.5;
    let below = top + rows + 1;
    let picture = Rect::new(area.x, below, area.width, scene_rows);
    scene(buf, picture, theme, &Scene { laid, drop: None, walker, cheering, flag: theme.accent, time: app.time });
    if t > walk_from {
        let words = Rect::new(area.x + 2, below + scene_rows + 1, area.width - 4, area.height.saturating_sub(below + scene_rows + 2));
        if words.height >= 6 {
            label(buf, Rect { height: 4, ..words }, "Build bridges around the world!", theme.text, 1);
            centered(buf, words, words.y + 5, "Press any key or click to start", theme.dim);
        } else if words.height >= 1 {
            centered(buf, words, words.y, "Build bridges around the world!  Press any key or click to start", theme.text);
        }
    }
}

/// The settings every screen offers, as buttons in a row. Flat ones name their key
/// first; tall ones put it on a line of its own.
fn settings(buf: &mut Buffer, app: &mut App, r: Rect, last: (&str, &str, Action)) {
    let theme = app.theme();
    let on = |b: bool| if b { "on" } else { "off" };
    // Each with a shorter way of saying it for a narrow window, which still shows
    // which theme it is and what is switched off.
    let items: [(&str, String, String, Action); 5] = [
        ("T", format!("Theme: {}", theme.name), theme.name.into(), Action::Theme),
        ("S", format!("Sound: {}", on(app.sound)), if app.sound { "Sound" } else { "Sound off" }.into(), Action::Sound),
        ("M", format!("Motion: {}", on(app.animations)), if app.animations { "Motion" } else { "Motion off" }.into(), Action::Motion),
        ("?", "Help".into(), "Help".into(), Action::Help),
        (last.0, last.1.into(), last.1.into(), last.2),
    ];
    let each = (r.width / items.len() as u16).min(24);
    let x0 = r.x + (r.width - each * items.len() as u16) / 2;
    for (i, (key, name, short, action)) in items.into_iter().enumerate() {
        let at = Rect::new(x0 + i as u16 * each, r.y, each - 1, r.height);
        let bg = mix(theme.panel, theme.answers[i % 4], 0.35);
        let inside = button(buf, app, at, Some(action), bg, false);
        let long = format!("{key}  {name}");
        let text = if long.chars().count() as u16 <= inside.width { long } else { format!("{key} {short}") };
        label(buf, inside, &text, ink(bg), 0);
    }
}

fn home(buf: &mut Buffer, app: &mut App, area: Rect) {
    let theme = app.theme();
    let (w, h) = (area.width, area.height);
    let scale = if w >= 100 && h >= 52 { 2 } else { 1 };
    title(buf, Rect::new(area.x, 0, w, title_rows(scale)), theme, scale, app.time, |_| Some(0.0));
    let mut y = title_rows(scale);
    let (earned, possible) = app.stars();
    centered(buf, area, y, &format!("★ {earned} of {possible} stars  ·  {}", app.rank()), theme.text);
    y += 2;

    let tall = h >= 50;
    let footer = if tall { 3 } else { 1 };
    let footer_y = h - footer - u16::from(tall);
    settings(buf, app, Rect::new(area.x + 1, footer_y, w - 2, footer), ("Q", "Quit", Action::Quit));
    let hint_y = footer_y - 1 - u16::from(tall);
    let hint = match app.problems.len() {
        0 => "Pick a square: click it, or use the arrow keys and Enter".to_string(),
        n => format!("{n} of your own question files could not be read: run 'fungeo check'"),
    };
    centered(buf, area, hint_y, &hint, theme.dim);

    // The grid: a row for each category, a column for each level. With room, the
    // names get big letters, which need a wide first column.
    let rows = app.cats.len() as u16;
    let grid_w = (w - 4).min(180);
    let x0 = area.x + (w - grid_w) / 2;
    let name_w = if grid_w >= 140 { (grid_w * 38 / 100).max(60) } else { (grid_w * 34 / 100).max(14) };
    let cell_w = (grid_w - name_w) / 3;
    let levels = if h >= 46 && cell_w >= 37 { font::ROWS } else { 1 };
    let space = hint_y.saturating_sub(y + levels + 1);
    let row_h = (space / rows).clamp(2, 7).min(space.max(1));
    let shown = (space / row_h).clamp(1, rows);
    let first = (app.cursor.0 as u16 + 1).saturating_sub(shown);
    let gap = u16::from(row_h >= 3);
    let high = row_h - gap;
    // All the names in big letters, or none of them.
    let name_at = |top: u16| Rect::new(x0 + 2, top, name_w.saturating_sub(5), high);
    // Big letters want a row to spare, or they touch the edges.
    let big_names = usize::from(high > font::ROWS && app.cats.iter().all(|c| big_lines(&c.name, name_at(0), 1).is_some()));
    let big_stars = high > font::ROWS && cell_w >= 28;
    for (i, level) in Level::ALL.into_iter().enumerate() {
        label(buf, Rect::new(x0 + name_w + i as u16 * cell_w, y, cell_w - 1, levels), level.name(), theme.text, 1);
    }
    if shown < rows {
        put(buf, x0, y, if first + shown < rows { "▼ more below" } else { "▲ more above" }, theme.dim, name_w);
    }
    y += levels + u16::from(levels > 1);
    for row in first..first + shown {
        let cat = &app.cats[row as usize];
        let (id, name, tint) = (cat.id.clone(), cat.name.clone(), cat.color);
        let playable: Vec<bool> = Level::ALL.iter().map(|&l| cat.has(l)).collect();
        let top = y + (row - first) * row_h;
        fill(buf, Rect::new(x0, top, name_w - 1, high), tint);
        label(buf, name_at(top), &name, ink(tint), big_names);
        for (i, level) in Level::ALL.into_iter().enumerate() {
            let at = Rect::new(x0 + name_w + i as u16 * cell_w, top, cell_w - 1, high);
            let earned = app.progress.stars(&id, level);
            if !playable[i] {
                fill(buf, at, mix(theme.bg, theme.dim, 0.15));
                label(buf, at, "-", theme.dim, 0);
                // The marker can rest here, though there is nothing to start.
                if app.cursor == (row as usize, i) {
                    put(buf, at.x + 1, at.y + high / 2, "▶", theme.dim, 1);
                    put(buf, at.right() - 2, at.y + high / 2, "◀", theme.dim, 1);
                }
                continue;
            }
            // Paler until it has been played, and lit up under the marker, which is a
            // pair of arrows so that the stars have the whole square.
            let marked = app.cursor == (row as usize, i);
            let bg = if earned > 0 { tint } else { mix(tint, theme.bg, 0.5) };
            let bg = if marked { mix(bg, (255, 255, 255), 0.3) } else { bg };
            fill(buf, at, bg);
            app.buttons.push((at, Action::Start(row as usize, level)));
            let inside = Rect { x: at.x + 2, width: at.width.saturating_sub(4), ..at };
            stars(buf, inside, earned, ink(bg), mix(bg, ink(bg), 0.25), big_stars);
            if marked && high >= if big_stars { 6 } else { 3 } {
                frame(buf, at, ink(bg));
            } else if marked {
                put(buf, at.x + 1, at.y + high / 2, "▶", ink(bg), 1);
                put(buf, at.right() - 2, at.y + high / 2, "◀", ink(bg), 1);
            }
        }
    }
}

/// Where everything in a round goes.
struct PlayLayout {
    header: Rect,
    scene: Rect,
    question: Rect,
    picture: Rect,
    answers: [Rect; 4],
    feedback: Rect,
    /// Big letters for the question, and for the answers.
    big: (bool, bool),
    columns: usize,
}

/// One way of fitting a round into the window, in rows of cells.
#[derive(Clone, Copy)]
struct Fit {
    question: u16,
    /// Big letters for the question, and for the answers.
    big: (bool, bool),
    /// The height of an answer's button.
    button: u16,
    columns: u16,
    picture: u16,
    /// Between the parts.
    gap: u16,
    /// The fewest the scene may have.
    scene: u16,
}

fn play_layout(area: Rect, asked: &Asked) -> PlayLayout {
    let (w, h) = (area.width, area.height);
    let header = if h >= 40 { 3 } else { 1 };
    let feedback = if h >= 40 { 7 } else { 4 };
    let inner = (w - 4).min(200);
    let x0 = area.x + (w - inner) / 2;
    let half = (inner - 2) / 2;
    let picture = if asked.question.picture.is_some() { (h / 5).clamp(5, 10) } else { 0 };

    let plain_question = wrap(&asked.question.text, (inner as usize).min(76)).len() as u16;
    let longest = asked.options.iter().map(|o| o.chars().count()).max().unwrap_or(0) as u16;
    let plain_columns = if half >= longest + 8 { 2 } else { 1 };
    // In big letters an answer has its letter, a gap and its words, inside the frame.
    let big_answer = asked.options.iter().map(|o| if font::supported(o) { font::width(o) as u16 + 14 } else { u16::MAX }).max().unwrap_or(u16::MAX);
    let big_question = font::wrap(&asked.question.text, inner as usize).map(|lines| lines.len() as u16 * font::ROWS).filter(|&rows| rows <= 12);

    // The ways to lay it out, from best to barely fitting. The question is the first
    // thing to get big letters: it is the most there is to read.
    let plain = |button: u16, picture: u16, gap: u16, scene: u16| Fit {
        question: plain_question,
        big: (false, false),
        button,
        columns: plain_columns,
        picture,
        gap,
        scene,
    };
    let mut tries: Vec<Fit> = Vec::new();
    if let Some(rows) = big_question {
        for columns in [2, 1] {
            if big_answer <= if columns == 2 { half } else { inner } {
                tries.push(Fit { question: rows, big: (true, true), button: 6, columns, picture, gap: 1, scene: 7 });
            }
        }
        tries.push(Fit { question: rows, big: (true, false), ..plain(5, picture, 1, 7) });
        tries.push(Fit { question: rows, big: (true, false), ..plain(3, picture, 1, 6) });
    }
    tries.extend([plain(5, picture, 1, 7), plain(3, picture.min(6), 1, 7), plain(3, picture.min(5), 0, 5), plain(1, picture.min(4), 0, 4)]);
    let needs = |t: &Fit| {
        let lines = 4 / t.columns;
        header + t.scene + t.question + t.picture + lines * t.button + (lines - 1) * t.gap + feedback + 3 * t.gap + u16::from(t.picture > 0) * t.gap
    };
    let pick = tries.iter().find(|t| needs(t) <= h).unwrap_or(&tries[tries.len() - 1]);
    let &Fit { question: question_rows, big, button: button_rows, columns, picture, gap, scene: least } = pick;

    let spare = h.saturating_sub(needs(pick));
    // Spare rows go to the scene first, but less of them when there is a flag to show:
    // the bigger a flag is, the plainer its star or its circle.
    let scene_rows = (least + spare)
        .min(if picture > 0 {
            9
        } else if h >= 50 {
            14
        } else {
            11
        })
        .max(least);
    let left = least + spare - scene_rows;
    let grow = if picture > 0 { left.min(16u16.saturating_sub(picture)) } else { 0 };
    let picture = picture + grow;
    // What is still spare goes between the parts, most of it around the question.
    let spare = (left - grow).min(9);
    let mut y = area.y + header;
    let scene = Rect::new(area.x, y, w, scene_rows);
    y += scene_rows + gap + spare / 3;
    let question = Rect::new(x0, y, inner, question_rows);
    y += question_rows + if picture > 0 { gap } else { 0 };
    let picture = Rect::new(x0, y, inner, picture);
    y += picture.height + gap + spare / 3;
    let button_w = if columns == 2 { half } else { inner };
    let mut answers = [Rect::default(); 4];
    for (i, at) in answers.iter_mut().enumerate() {
        let (column, line) = (i as u16 % columns, i as u16 / columns);
        *at = Rect::new(x0 + column * (button_w + 2), y + line * (button_rows + gap), button_w, button_rows);
    }
    let feedback = Rect::new(x0, (area.y + h).saturating_sub(feedback), inner, feedback);
    PlayLayout { header: Rect::new(area.x, area.y, w, header), scene, question, picture, answers, feedback, big, columns: columns as usize }
}

fn play(buf: &mut Buffer, app: &mut App, area: Rect) {
    let theme = app.theme();
    let Some((cat, level, round)) = &app.playing else { return };
    let (asked, laid, tint) = (round.asked.clone(), round.planks, app.cats[*cat].color);
    let built = round.built();
    let name = app.cats[*cat].name.clone();
    let headings =
        [format!("{name} · {} · {laid} of {PLANKS} planks", level.name()), format!("{name} · {} · {laid}/{PLANKS}", level.name()), format!("{laid}/{PLANKS}")];
    let lay = play_layout(area, &asked);
    app.columns = lay.columns;

    // The top line: back, where we are, and the settings.
    fill(buf, lay.header, theme.panel);
    let back = Rect::new(lay.header.x, lay.header.y, 14, lay.header.height);
    let inside = button(buf, app, back, Some(Action::Back), mix(theme.panel, theme.accent, 0.4), false);
    label(buf, inside, "Esc  Back", ink(mix(theme.panel, theme.accent, 0.4)), 0);
    let keys: [(&str, Action); 4] = [("T Theme", Action::Theme), ("S Sound", Action::Sound), ("M Motion", Action::Motion), ("? Help", Action::Help)];
    let each: u16 = if area.width >= 100 { 13 } else { 4 };
    let keys_x = lay.header.right() - each * 4;
    for (i, (name, action)) in keys.into_iter().enumerate() {
        let at = Rect::new(keys_x + i as u16 * each, lay.header.y, each - 1, lay.header.height);
        let off = (action == Action::Sound && !app.sound) || (action == Action::Motion && !app.animations);
        let bg = if off { mix(theme.panel, theme.dim, 0.3) } else { mix(theme.panel, theme.answers[i], 0.4) };
        let inside = button(buf, app, at, Some(action), bg, false);
        if each > 4 {
            label(buf, inside, name, ink(bg), 0);
        } else {
            put(buf, at.x + 1, at.y + at.height / 2, &name[..1], ink(bg), 1);
        }
    }
    let middle = Rect::new(back.right() + 1, lay.header.y + lay.header.height / 2, keys_x.saturating_sub(back.right() + 2), 1);
    let heading = headings.iter().find(|h| h.chars().count() as u16 <= middle.width).unwrap_or(&headings[2]);
    centered(buf, middle, middle.y, heading, theme.text);

    // A plank still falling is not lying on the bridge yet.
    let falling = app.drop.as_ref().map(|d| (d.slot, d.good, d.age));
    let lying = laid - usize::from(falling.is_some_and(|(_, good, _)| good));
    let cheering = matches!(app.phase, Phase::Done { .. }) && app.animations;
    scene(buf, lay.scene, theme, &Scene { laid: lying, drop: falling, walker: app.walker, cheering, flag: tint, time: app.time });

    if let Phase::Done { stars: earned, best } = app.phase {
        return done(buf, app, Rect::new(area.x, lay.scene.bottom(), area.width, area.bottom() - lay.scene.bottom()), earned, best);
    }

    if app.phase == Phase::Crossing {
        // Nothing left to answer: watch the explorer go, or press on to the stars.
        let below = Rect::new(area.x + 2, lay.scene.bottom() + 1, area.width - 4, area.bottom() - lay.scene.bottom() - 1);
        app.buttons.push((area, Action::Skip));
        return label(buf, Rect { height: below.height.min(8), ..below }, "Off we go!", theme.good, 1);
    }

    label(buf, lay.question, &asked.question.text, theme.text, usize::from(lay.big.0));
    if let Some(picture) = &asked.question.picture {
        flag(buf, lay.picture, picture, theme);
    }

    let answered = match app.phase {
        Phase::Answered { picked, right } => Some((picked, right)),
        _ => None,
    };
    for (i, option) in asked.options.iter().enumerate() {
        let mut at = lay.answers[i];
        let right = i == asked.correct;
        let (bg, mark) = match answered {
            None => (theme.answers[i], ""),
            Some(_) if right => (theme.good, "✓ "),
            Some((picked, _)) if picked == i => (theme.bad, "✗ "),
            Some(_) => (mix(theme.answers[i], theme.bg, 0.65), ""),
        };
        // A wrong pick shakes its head.
        if answered == Some((i, false)) && app.animations && app.phase_time < 0.4 {
            at.x = (at.x as i32 + if (app.phase_time * 20.0) as i32 % 2 == 0 { 1 } else { -1 }).max(0) as u16;
        }
        let marked = if answered.is_some() { right } else { app.focus == i };
        let action = answered.is_none().then_some(Action::Answer(i));
        let inside = button(buf, app, at, action, bg, marked);
        let fg = ink(bg);
        let letter = ["A", "B", "C", "D"][i];
        if lay.big.1 && inside.height >= font::ROWS {
            let top = inside.y as i32 * 2 + (inside.height as i32 * 2 - 7) / 2;
            font::draw(buf, inside.x as i32, top, letter, color(mix(bg, fg, 0.55)), 1);
            label(buf, Rect { x: inside.x + 8, width: inside.width.saturating_sub(8), ..inside }, option, fg, 1);
        } else {
            let line = inside.y + inside.height / 2;
            put(buf, inside.x, line, letter, mix(bg, fg, 0.6), 1);
            label(buf, Rect { x: inside.x + 3, width: inside.width.saturating_sub(3), ..inside }, &format!("{mark}{option}"), fg, 0);
        }
    }

    // Under the answers: what to do, or how it went and something worth knowing.
    let fb = lay.feedback;
    let Some((_, right)) = answered else {
        let hint = if laid == 0 {
            "Every right answer adds a plank to the bridge. Click an answer, or press A, B, C or D."
        } else {
            "Click an answer, or press A, B, C or D."
        };
        label(buf, Rect { y: fb.y + fb.height / 2, height: fb.height - fb.height / 2, ..fb }, hint, theme.dim, 0);
        return;
    };
    let next_w = 26.min(fb.width / 3);
    let words = Rect { width: fb.width - next_w - 2, ..fb };
    let said = if right { app.praise.to_string() } else { format!("{} It is {}.", app.praise, asked.question.answer) };
    let tone = if right { theme.good } else { theme.bad };
    let fact_y = if fb.height >= 7 {
        label(buf, Rect { height: 4, ..words }, &said, tone, 1);
        words.y + 4
    } else {
        label(buf, Rect { height: 1, ..words }, &said, tone, 0);
        words.y + 1
    };
    label(buf, Rect { y: fact_y, height: fb.bottom() - fact_y - 1, ..words }, &asked.question.fact, theme.text, 0);
    let next = Rect::new(fb.right() - next_w, fb.y + (fb.height - 3.min(fb.height)) / 2, next_w, 3.min(fb.height));
    let inside = button(buf, app, next, Some(Action::Next), theme.accent, true);
    label(buf, inside, if built { "Enter  Cross! →" } else { "Enter  Next →" }, ink(theme.accent), 0);
}

/// The end of a round, under the scene: the stars, and what to do next.
fn done(buf: &mut Buffer, app: &mut App, r: Rect, earned: u8, best: bool) {
    let theme = app.theme();
    let r = Rect { x: r.x + 2, width: r.width - 4, ..r };
    let roomy = r.height >= 18;
    let mut y = r.y + 1;
    label(buf, Rect::new(r.x, y, r.width, font::ROWS), "Bridge built!", theme.good, 1);
    y += font::ROWS + 1;
    let star_rows = if roomy { font::ROWS } else { 1 };
    stars(buf, Rect::new(r.x, y, r.width, star_rows), app.stars_shown, (255, 200, 30), mix(theme.bg, theme.dim, 0.5), roomy);
    y += star_rows + 1;
    let said = match (earned, best) {
        (3, true) => "Three stars! A new stamp for your passport.",
        (_, true) => "A new stamp for your passport. Can you get three stars?",
        (3, false) => "Three stars again!",
        _ => "You crossed! Fewer mistakes earn more stars.",
    };
    centered(buf, r, y, said, theme.text);
    y += 2;
    let high = if r.bottom().saturating_sub(y) >= 4 { 3 } else { 1 };
    let wide = 30.min((r.width - 2) / 2);
    let x = r.x + (r.width - wide * 2 - 2) / 2;
    for (i, (name, bg)) in [("Enter  Passport", theme.answers[1]), ("P  Play again", theme.answers[3])].into_iter().enumerate() {
        let (words, action) = (if app.focus == i { name.to_string() } else { name.replace("Enter  ", "") }, App::DONE[i]);
        let inside = button(buf, app, Rect::new(x + i as u16 * (wide + 2), y, wide, high), Some(action), bg, app.focus == i);
        label(buf, inside, &words, ink(bg), 0);
    }
}

const HELP: [&str; 13] = [
    "Build a bridge across the river!",
    "",
    "Every right answer adds a plank. Eight planks and you can cross.",
    "A wrong answer costs nothing but a plank in the river,",
    "and that question will come back for another try.",
    "Cross with one mistake or none to earn three stars.",
    "",
    "Mouse: click anything.",
    "Keys: arrows and Enter, or A B C D (or 1 2 3 4) for an answer.",
    "T theme · S sound · M motion · Esc back · Q quit",
    "",
    "In a big window the words are drawn in big letters.",
    "Any key or click closes this.",
];

fn help(buf: &mut Buffer, app: &mut App, area: Rect) {
    let theme = app.theme();
    let wide = (HELP.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16 + 6).min(area.width);
    let high = (HELP.len() as u16 + 2).min(area.height);
    let r = Rect::new(area.x + (area.width - wide) / 2, area.y + (area.height - high) / 2, wide, high);
    app.buttons.push((area, Action::Help));
    let inside = button(buf, app, r, None, theme.panel, true);
    for (i, line) in HELP.iter().enumerate().take(inside.height as usize) {
        centered(buf, inside, inside.y + i as u16, line, if i == 0 { theme.accent } else { theme.text });
    }
}

/// The confetti, which falls behind everything, or the sparkles, which fly in front.
fn particles(buf: &mut Buffer, app: &App, front: bool) {
    for p in app.particles.iter().filter(|p| p.front == front && p.x >= 0.0 && p.y >= 0.0) {
        if let Some(cell) = buf.cell_mut((p.x as u16, p.y as u16)) {
            // Never over a letter or a picture.
            if cell.symbol() == " " {
                cell.set_symbol(p.symbol).set_fg(color(p.color));
            }
        }
    }
}

/// The nearest of the 256 colors every terminal has, for those without all of them.
fn indexed(c: Color) -> Color {
    let Color::Rgb(r, g, b) = c else { return c };
    let step = |v: u8| (v as u16 * 5 + 127) / 255;
    Color::Indexed((16 + 36 * step(r) + 6 * step(g) + step(b)) as u8)
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let buf = frame.buffer_mut();
    let theme = app.theme();
    app.buttons.clear();
    app.size = (area.width, area.height);
    fill(buf, area, theme.bg);
    if area.width < MIN.0 || area.height < MIN.1 {
        label(buf, area, &format!("Please make the window bigger: at least {} by {}.", MIN.0, MIN.1), theme.text, 0);
    } else {
        particles(buf, app, false);
        match app.screen {
            Screen::Intro => intro(buf, app, area),
            Screen::Home => home(buf, app, area),
            Screen::Play => play(buf, app, area),
        }
        particles(buf, app, true);
        if app.help {
            help(buf, app, area);
        }
    }
    if !app.truecolor {
        for cell in &mut buf.content {
            cell.fg = indexed(cell.fg);
            cell.bg = indexed(cell.bg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quiz;
    use crate::theme::{Settings, THEMES};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn app() -> App {
        App::new(true, Settings::default(), quiz::builtin(), Vec::new())
    }

    /// Draws the app in a window of this size and returns its lines of text.
    fn screen(app: &mut App, w: u16, h: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        let buf = terminal.backend().buffer();
        (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect()
    }

    fn has(lines: &[String], text: &str) -> bool {
        lines.iter().any(|l| l.contains(text))
    }

    const SIZES: [(u16, u16); 7] = [(60, 22), (80, 24), (100, 30), (120, 40), (160, 45), (213, 58), (300, 90)];

    #[test]
    fn every_screen_draws_at_every_size_in_every_theme() {
        for (w, h) in SIZES {
            for theme in 0..THEMES.len() {
                let mut app = app();
                app.theme = theme;
                app.begin();
                for _ in 0..40 {
                    app.advance(0.1);
                    screen(&mut app, w, h);
                }
                app.act(Action::Skip);
                let lines = screen(&mut app, w, h);
                // In a big window the names of the levels are big letters, not text.
                assert!(has(&lines, "stars") && (h >= 46 || has(&lines, "Easy") && has(&lines, "Hard")), "{w}x{h}: {lines:#?}");
                // Every level of every category that is on the screen can be clicked.
                let cells = app.buttons.iter().filter(|(_, a)| matches!(a, Action::Start(..))).count();
                assert!(cells >= 3 && cells % 3 == 0, "{w}x{h}: {cells} squares");
                assert!(app.buttons.iter().all(|(r, _)| r.right() <= w && r.bottom() <= h && r.width > 0 && r.height > 0), "{w}x{h}");

                for cat in 0..app.cats.len() {
                    app.start(cat, Level::ALL[cat % 3]);
                    let lines = screen(&mut app, w, h);
                    assert!(has(&lines, "0 of 8 planks") || has(&lines, "· 0/8"), "{w}x{h}: {lines:#?}");
                    let answers: Vec<Rect> = app.buttons.iter().filter(|(_, a)| matches!(a, Action::Answer(_))).map(|(r, _)| *r).collect();
                    assert_eq!(answers.len(), 4);
                    // No answer under another, and none off the window or over the scene.
                    for (i, a) in answers.iter().enumerate() {
                        assert!(a.right() <= w && a.bottom() <= h && a.height >= 1, "{w}x{h}: {a:?}");
                        assert!(answers[..i].iter().all(|b| !a.intersects(*b)), "{w}x{h}: {answers:?}");
                    }
                    app.act(Action::Answer(0));
                    app.advance(0.2);
                    screen(&mut app, w, h);
                    assert!(app.buttons.iter().any(|(_, a)| *a == Action::Next), "{w}x{h}");
                    app.help = true;
                    assert!(has(&screen(&mut app, w, h), "Build a bridge across the river!"));
                    app.help = false;
                }
            }
        }
    }

    #[test]
    fn a_whole_round_is_drawn_to_its_end() {
        for (w, h) in SIZES {
            let mut app = app();
            app.start(1, Level::Easy);
            loop {
                let (_, _, round) = app.playing.as_ref().unwrap();
                let right = round.asked.correct;
                app.act(Action::Answer(right));
                for _ in 0..6 {
                    app.advance(0.1);
                    screen(&mut app, w, h);
                }
                let built = app.playing.as_ref().unwrap().2.built();
                assert!(has(&screen(&mut app, w, h), if built { "Cross!" } else { "Next" }), "{w}x{h}");
                app.act(Action::Next);
                if built {
                    break;
                }
            }
            for _ in 0..40 {
                app.advance(0.1);
                screen(&mut app, w, h);
            }
            let lines = screen(&mut app, w, h);
            assert!(has(&lines, "Play again") && has(&lines, "Passport") && has(&lines, "passport"), "{w}x{h}: {lines:#?}");
            for action in App::DONE {
                assert!(app.buttons.iter().any(|(r, a)| *a == action && r.bottom() <= h), "{w}x{h}: {action:?}");
            }
        }
    }

    #[test]
    fn big_letters_in_a_big_window_and_plain_ones_in_a_small() {
        let mut app = app();
        app.animations = false;
        app.start(0, Level::Easy);
        let text = app.playing.as_ref().unwrap().2.asked.question.text.clone();
        // Small: the question is there as ordinary text.
        assert!(has(&screen(&mut app, 80, 24), &text));
        // Big: it is drawn out of blocks instead, and so are the answers.
        let lines = screen(&mut app, 213, 58);
        assert!(!has(&lines, &text));
        let lay = play_layout(Rect::new(0, 0, 213, 58), &app.playing.as_ref().unwrap().2.asked);
        assert_eq!(lay.big, (true, true));
        let blocks = |r: Rect| {
            (r.top()..r.bottom())
                .flat_map(|y| lines[y as usize].chars().skip(r.x as usize).take(r.width as usize).collect::<Vec<_>>())
                .filter(|c| "█▀▄".contains(*c))
                .count()
        };
        assert!(blocks(lay.question) > 100 && lay.answers.iter().all(|&a| blocks(a) > 40));
    }

    #[test]
    fn a_flag_question_shows_its_flag() {
        let mut app = app();
        app.animations = false;
        let flags = app.cats.iter().position(|c| c.id == "flags").unwrap();
        app.start(flags, Level::Easy);
        for (w, h) in SIZES {
            let asked = app.playing.as_ref().unwrap().2.asked.clone();
            let lay = play_layout(Rect::new(0, 0, w, h), &asked);
            assert!(lay.picture.height >= 4 && lay.picture.bottom() <= lay.answers[0].y, "{w}x{h}: {lay:?}", lay = lay.picture);
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|f| draw(f, &mut app)).unwrap();
            let buf = terminal.backend().buffer();
            // The middle of the picture is one of the flag's own colors.
            assert!(asked.question.picture.is_some());
            let middle = &buf[(lay.picture.x + lay.picture.width / 2, lay.picture.y + lay.picture.height / 2)];
            assert_eq!(middle.symbol(), "▀", "{w}x{h}");
        }
    }

    #[test]
    fn too_small_a_window_says_so_and_fewer_colors_are_used_when_asked() {
        let mut app = app();
        assert!(has(&screen(&mut app, 40, 10), "Please make the window bigger"));
        assert!(app.buttons.is_empty());
        app.truecolor = false;
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        assert!(terminal.backend().buffer().content.iter().all(|c| !matches!(c.fg, Color::Rgb(..)) && !matches!(c.bg, Color::Rgb(..))));
        assert_eq!(indexed(Color::Rgb(255, 255, 255)), Color::Indexed(231));
        assert_eq!(indexed(Color::Rgb(0, 0, 0)), Color::Indexed(16));
    }

    #[test]
    fn many_categories_scroll_with_the_marker() {
        let mut cats = quiz::builtin();
        for i in 0..12 {
            let mut extra = cats[0].clone();
            extra.id = format!("extra{i}");
            extra.name = format!("Extra {i}");
            cats.push(extra);
        }
        let mut app = App::new(true, Settings::default(), cats, vec!["a problem".into()]);
        let lines = screen(&mut app, 80, 24);
        assert!(has(&lines, "more below") && !has(&lines, "Extra 11") && has(&lines, "could not be read"));
        app.cursor.0 = app.cats.len() - 1;
        let lines = screen(&mut app, 80, 24);
        assert!(has(&lines, "more above") && has(&lines, "Mix"));
    }
}
