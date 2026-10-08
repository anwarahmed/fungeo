//! `App`: where the player is, what every key, click and tick of the clock does, and
//! the stars that are remembered between runs. Draws nothing.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

use crate::quiz::{Category, Level, PLANKS, Question, Rng, Round};
use crate::sound::{Sound, Speaker};
use crate::theme::{Rgb, Settings, THEMES, Theme};

/// Seconds a plank takes to fall onto the bridge; the knock in `Sound::Right` comes
/// at the same moment.
pub const DROP: f32 = 0.45;
/// Seconds a plank that missed is seen falling and sinking.
pub const SINK: f32 = 1.1;
/// Seconds between the letters of the title landing in the intro; the notes of
/// `Sound::Intro` are this far apart.
pub const INTRO_LETTER: f32 = 0.25;
/// Seconds the intro lasts when nobody skips it.
pub const INTRO: f32 = 5.5;
/// Seconds from the end of a round to its first star, and from one star to the next.
const STAR_EVERY: f32 = 0.45;
/// Questions dealt to a round: the planks, and some to spare for wrong answers.
const HAND: usize = PLANKS + 4;

/// Everything a button can do. Keys do the same things through `App::act`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Play this category (an index into `App::cats`) at this level.
    Start(usize, Level),
    Answer(usize),
    /// On from an answered question, to the next or across the bridge.
    Next,
    /// Back to the passport.
    Back,
    /// The same category and level once more.
    Again,
    Theme,
    Sound,
    Motion,
    Help,
    Quit,
    /// Past the intro, or past the walk across the bridge.
    Skip,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Intro,
    /// The passport: every category and level, and the stars earned in each.
    Home,
    Play,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Asking,
    Answered {
        picked: usize,
        right: bool,
    },
    /// The bridge is built and the explorer is walking over it.
    Crossing,
    Done {
        stars: u8,
        /// More stars than this category and level had before.
        best: bool,
    },
}

/// The plank of the last answer, on its way down.
pub struct Drop {
    pub slot: usize,
    /// Whether it lands on the bridge or falls in the river.
    pub good: bool,
    pub age: f32,
}

/// A bit of confetti or a sparkle, placed in cells.
pub struct Particle {
    pub x: f32,
    pub y: f32,
    vx: f32,
    vy: f32,
    gravity: f32,
    pub age: f32,
    pub life: f32,
    pub color: Rgb,
    pub symbol: &'static str,
    /// Drawn over the screen, as sparkles are, or behind it, as confetti is.
    pub front: bool,
}

/// The most stars earned in each category and level since the game was started. They
/// are not kept: every start is a fresh passport, which is what the user asked for.
#[derive(Default, PartialEq, Debug)]
pub struct Progress(HashMap<String, u8>);

impl Progress {
    fn key(id: &str, level: Level) -> String {
        format!("{id}.{}", level.name().to_lowercase())
    }

    pub fn stars(&self, id: &str, level: Level) -> u8 {
        self.0.get(&Progress::key(id, level)).copied().unwrap_or(0)
    }

    /// Notes a finished round, and says whether it beat what was there.
    pub fn earn(&mut self, id: &str, level: Level, stars: u8) -> bool {
        let best = self.0.entry(Progress::key(id, level)).or_insert(0);
        let better = stars > *best;
        *best = (*best).max(stars);
        better
    }
}

pub struct App {
    /// Every category, and last of all "Mix", which is all of them together.
    pub cats: Vec<Category>,
    /// Question files of the player's own that could not be read.
    pub problems: Vec<String>,
    pub progress: Progress,
    pub settings_path: Option<PathBuf>,
    pub screen: Screen,
    /// The help is shown over whatever screen this is.
    pub help: bool,
    /// The marker on the passport: a row of `cats` and a column of `Level::ALL`.
    pub cursor: (usize, usize),
    /// The marker in a round: one of the four answers, or of the two buttons at the end.
    pub focus: usize,
    /// What is being played: the category, the level and the round itself.
    pub playing: Option<(usize, Level, Round)>,
    pub phase: Phase,
    /// Seconds since the phase began, or since the intro did.
    pub phase_time: f32,
    /// Seconds things have been moving for; it stands still while animations are off.
    pub time: f32,
    /// Where the explorer is, in planks from the near bank.
    pub walker: f32,
    pub drop: Option<Drop>,
    pub particles: Vec<Particle>,
    /// Stars of a finished round that have appeared so far.
    pub stars_shown: u8,
    /// What was said about the last answer.
    pub praise: &'static str,
    /// What can be clicked, as of the last time the screen was drawn; later ones are
    /// on top of earlier ones.
    pub buttons: Vec<(Rect, Action)>,
    /// How many answers the screen shows side by side, for the arrow keys.
    pub columns: usize,
    /// The window in cells, for the confetti.
    pub size: (u16, u16),
    pub theme: usize,
    pub sound: bool,
    pub animations: bool,
    auto_update: bool,
    pub truecolor: bool,
    pub speaker: Speaker,
    pub quit: bool,
    rng: Rng,
    /// The questions of each category and level not yet dealt, so that a second round
    /// brings new ones.
    decks: HashMap<(usize, Level), Vec<Question>>,
    last_tick: Option<Instant>,
    step_in: f32,
    /// The mouse button went down and has not come up yet.
    button_down: bool,
    intro_burst: bool,
}

const PRAISE: [&str; 6] = ["Yes!", "Great!", "Well done!", "Super!", "You got it!", "Brilliant!"];
const COMFORT: [&str; 3] = ["Not quite.", "Good try!", "Almost!"];

impl App {
    pub fn new(truecolor: bool, settings: Settings, mut cats: Vec<Category>, problems: Vec<String>) -> App {
        let everything = cats.iter().flat_map(|c| c.questions.iter().cloned()).collect();
        cats.push(Category { id: "mix".into(), name: "Mix".into(), color: (150, 105, 225), order: i32::MAX, questions: everything });
        App {
            cats,
            problems,
            progress: Progress::default(),
            settings_path: None,
            screen: Screen::Home,
            help: false,
            cursor: (0, 0),
            focus: 0,
            playing: None,
            phase: Phase::Asking,
            phase_time: 0.0,
            time: 0.0,
            walker: 0.0,
            drop: None,
            particles: Vec::new(),
            stars_shown: 0,
            praise: "",
            buttons: Vec::new(),
            columns: 2,
            size: (80, 24),
            theme: settings.theme,
            sound: settings.sound,
            animations: settings.animations,
            auto_update: settings.update,
            truecolor,
            speaker: Speaker::silent(),
            quit: false,
            rng: Rng::from_clock(),
            decks: HashMap::new(),
            last_tick: None,
            step_in: 0.0,
            button_down: false,
            intro_burst: false,
        }
    }

    pub fn theme(&self) -> &'static Theme {
        &THEMES[self.theme]
    }

    /// Opens with the intro, unless nothing is to move.
    pub fn begin(&mut self) {
        if self.animations {
            self.screen = Screen::Intro;
            self.phase_time = 0.0;
            self.intro_burst = false;
            self.play(Sound::Intro);
        }
    }

    fn play(&mut self, sound: Sound) {
        if self.sound {
            self.speaker.play(sound);
        }
    }

    fn save_settings(&self) {
        if let Some(path) = &self.settings_path {
            Settings { theme: self.theme, update: self.auto_update, animations: self.animations, sound: self.sound }.save(path);
        }
    }

    /// Stars earned, and stars there are to earn.
    pub fn stars(&self) -> (usize, usize) {
        let mut total = (0, 0);
        for cat in &self.cats {
            for level in Level::ALL.into_iter().filter(|&l| cat.has(l)) {
                total.0 += self.progress.stars(&cat.id, level) as usize;
                total.1 += 3;
            }
        }
        total
    }

    /// What the player is called for the stars they have.
    pub fn rank(&self) -> &'static str {
        let (earned, possible) = self.stars();
        match earned * 100 / possible.max(1) {
            0 => "New Explorer",
            1..25 => "Pathfinder",
            25..50 => "Navigator",
            50..75 => "Adventurer",
            75..100 => "Globetrotter",
            _ => "World Champion",
        }
    }

    /// Up to `HAND` questions for a round: what the deck still holds, and when that
    /// runs out, everything shuffled afresh.
    fn deal(&mut self, cat: usize, level: Level) -> Vec<Question> {
        let mut deck = self.decks.remove(&(cat, level)).unwrap_or_default();
        let mut hand: Vec<Question> = Vec::new();
        for refill in [false, true] {
            if refill {
                deck = self.cats[cat].questions.iter().filter(|q| q.level == level).cloned().collect();
                self.rng.shuffle(&mut deck);
            }
            while hand.len() < HAND
                && let Some(question) = deck.pop()
            {
                if !hand.contains(&question) {
                    hand.push(question);
                }
            }
            if hand.len() >= HAND {
                break;
            }
        }
        self.decks.insert((cat, level), deck);
        hand
    }

    pub fn start(&mut self, cat: usize, level: Level) {
        if !self.cats.get(cat).is_some_and(|c| c.has(level)) {
            return;
        }
        let hand = self.deal(cat, level);
        self.playing = Some((cat, level, Round::new(hand, &mut self.rng)));
        self.cursor = (cat, Level::ALL.iter().position(|&l| l == level).unwrap_or(0));
        self.screen = Screen::Play;
        self.phase = Phase::Asking;
        self.phase_time = 0.0;
        self.walker = 0.0;
        self.drop = None;
        self.particles.clear();
        self.focus = 0;
        self.play(Sound::Start);
    }

    fn answer(&mut self, option: usize) {
        let Some((_, _, round)) = &mut self.playing else { return };
        if self.phase != Phase::Asking || option >= round.asked.options.len() {
            return;
        }
        let slot = round.planks;
        let right = round.answer(option);
        self.phase = Phase::Answered { picked: option, right };
        self.phase_time = 0.0;
        self.focus = option;
        let words: &[&'static str] = if right { &PRAISE } else { &COMFORT };
        self.praise = words[self.rng.below(words.len())];
        if self.animations {
            self.drop = Some(Drop { slot, good: right, age: 0.0 });
            if right && let Some(&(at, _)) = self.buttons.iter().find(|(_, action)| *action == Action::Answer(option)) {
                self.sparkle(at);
            }
        }
        self.play(if right { Sound::Right } else { Sound::Wrong });
    }

    fn next(&mut self) {
        let Some((_, _, round)) = &mut self.playing else { return };
        if !matches!(self.phase, Phase::Answered { .. }) {
            return;
        }
        self.phase_time = 0.0;
        if round.built() {
            self.phase = Phase::Crossing;
            self.step_in = 0.0;
            if !self.animations {
                self.finish();
            }
        } else {
            round.next(&mut self.rng);
            self.phase = Phase::Asking;
            self.focus = 0;
            self.play(Sound::Click);
        }
    }

    /// The explorer is over: count the stars and remember them.
    fn finish(&mut self) {
        let Some((cat, level, round)) = &self.playing else { return };
        let stars = round.stars();
        let best = self.progress.earn(&self.cats[*cat].id, *level, stars);
        self.phase = Phase::Done { stars, best };
        self.phase_time = 0.0;
        self.walker = PLANKS as f32 + 1.5;
        self.drop = None;
        self.stars_shown = if self.animations { 0 } else { stars };
        self.focus = 0;
        self.play(Sound::Win);
        self.confetti(90);
    }

    fn home(&mut self) {
        self.screen = Screen::Home;
        self.playing = None;
        self.phase = Phase::Asking;
        self.drop = None;
        self.particles.clear();
    }

    pub fn act(&mut self, action: Action) {
        match action {
            Action::Start(cat, level) => self.start(cat, level),
            Action::Answer(option) => self.answer(option),
            Action::Next => self.next(),
            Action::Back => {
                self.home();
                self.play(Sound::Click);
            }
            Action::Again => {
                if let Some((cat, level, _)) = self.playing {
                    self.start(cat, level);
                }
            }
            Action::Theme => {
                self.theme = (self.theme + 1) % THEMES.len();
                self.save_settings();
                self.play(Sound::Click);
            }
            Action::Sound => {
                self.sound = !self.sound;
                self.save_settings();
                self.play(Sound::Click);
            }
            Action::Motion => {
                self.animations = !self.animations;
                self.save_settings();
                self.play(Sound::Click);
                if !self.animations {
                    self.drop = None;
                    self.particles.clear();
                    match (self.screen, self.phase) {
                        (Screen::Intro, _) => self.home(),
                        (Screen::Play, Phase::Crossing) => self.finish(),
                        (Screen::Play, Phase::Done { stars, .. }) => self.stars_shown = stars,
                        _ => {}
                    }
                }
            }
            Action::Help => self.help = !self.help,
            Action::Quit => self.quit = true,
            Action::Skip => match (self.screen, self.phase) {
                (Screen::Intro, _) => self.home(),
                (Screen::Play, Phase::Crossing) => self.finish(),
                _ => {}
            },
        }
    }

    /// The two buttons under a finished round, in the order they are drawn.
    pub const DONE: [Action; 2] = [Action::Back, Action::Again];

    pub fn on_key(&mut self, key: KeyEvent) {
        let code = match key.code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            other => other,
        };
        if key.modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.help {
            self.help = false;
            return;
        }
        if self.screen == Screen::Intro {
            return self.act(Action::Skip);
        }
        // The same everywhere. None of these letters is an answer's.
        match code {
            KeyCode::Char('t') => return self.act(Action::Theme),
            KeyCode::Char('s') => return self.act(Action::Sound),
            KeyCode::Char('m') => return self.act(Action::Motion),
            KeyCode::Char('?') | KeyCode::F(1) => return self.act(Action::Help),
            _ => {}
        }
        let confirm = matches!(code, KeyCode::Enter | KeyCode::Char(' '));
        if self.screen == Screen::Home {
            let (rows, columns) = (self.cats.len(), Level::ALL.len());
            match code {
                KeyCode::Up => self.cursor.0 = (self.cursor.0 + rows - 1) % rows,
                KeyCode::Down => self.cursor.0 = (self.cursor.0 + 1) % rows,
                KeyCode::Left => self.cursor.1 = (self.cursor.1 + columns - 1) % columns,
                KeyCode::Right => self.cursor.1 = (self.cursor.1 + 1) % columns,
                KeyCode::Tab => {
                    self.cursor.1 = (self.cursor.1 + 1) % columns;
                    if self.cursor.1 == 0 {
                        self.cursor.0 = (self.cursor.0 + 1) % rows;
                    }
                }
                KeyCode::Home => self.cursor = (0, 0),
                KeyCode::End => self.cursor.0 = rows - 1,
                _ if confirm => self.act(Action::Start(self.cursor.0, Level::ALL[self.cursor.1])),
                // Not Esc: it is the way back from everywhere else, and one press too many
                // must not close the game.
                KeyCode::Char('q') => self.act(Action::Quit),
                _ => {}
            }
            return;
        }
        if matches!(code, KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q')) {
            return self.act(Action::Back);
        }
        match self.phase {
            Phase::Asking => {
                let columns = self.columns.clamp(1, 2);
                let step = |focus: usize, by: isize| ((focus as isize + by).rem_euclid(4)) as usize;
                match code {
                    KeyCode::Char(c @ '1'..='4') => self.act(Action::Answer(c as usize - '1' as usize)),
                    KeyCode::Char(c @ 'a'..='d') => self.act(Action::Answer(c as usize - 'a' as usize)),
                    KeyCode::Left if columns == 2 => self.focus ^= 1,
                    KeyCode::Right if columns == 2 => self.focus ^= 1,
                    KeyCode::Up => self.focus = step(self.focus, -(columns as isize)),
                    KeyCode::Down => self.focus = step(self.focus, columns as isize),
                    KeyCode::Left | KeyCode::BackTab => self.focus = step(self.focus, -1),
                    KeyCode::Right | KeyCode::Tab => self.focus = step(self.focus, 1),
                    _ if confirm => self.act(Action::Answer(self.focus)),
                    _ => {}
                }
            }
            Phase::Answered { .. } => {
                if confirm || matches!(code, KeyCode::Right | KeyCode::Char('n') | KeyCode::Tab) {
                    self.act(Action::Next);
                }
            }
            Phase::Crossing => {
                if confirm {
                    self.act(Action::Skip);
                }
            }
            Phase::Done { .. } => match code {
                KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down | KeyCode::Tab | KeyCode::BackTab => self.focus ^= 1,
                KeyCode::Char('p') => self.act(Action::Again),
                _ if confirm => self.act(App::DONE[self.focus.min(1)]),
                _ => {}
            },
        }
    }

    pub fn on_mouse(&mut self, mouse: MouseEvent) {
        let at = Position::new(mouse.column, mouse.row);
        let under = self.buttons.iter().rev().find(|(rect, _)| rect.contains(at)).map(|&(_, action)| action);
        // A click counts when the button goes down. Should a terminal only ever report
        // it coming up, that counts instead. (The sister project funchess does the same.)
        let click = match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.button_down = true;
                true
            }
            MouseEventKind::Up(MouseButton::Left) => !std::mem::take(&mut self.button_down),
            _ => false,
        };
        match mouse.kind {
            _ if click => {
                if self.help {
                    self.help = false;
                } else if let Some(action) = under {
                    self.act(action);
                }
            }
            // The marker follows the pointer, so that there is one marker, not two.
            MouseEventKind::Moved => match under {
                Some(Action::Start(cat, level)) => self.cursor = (cat, Level::ALL.iter().position(|&l| l == level).unwrap_or(0)),
                Some(Action::Answer(option)) if self.phase == Phase::Asking => self.focus = option,
                Some(action) if matches!(self.phase, Phase::Done { .. }) => {
                    if let Some(i) = App::DONE.iter().position(|&a| a == action) {
                        self.focus = i;
                    }
                }
                _ => {}
            },
            MouseEventKind::ScrollUp if self.screen == Screen::Home && !self.help => self.cursor.0 = self.cursor.0.saturating_sub(1),
            MouseEventKind::ScrollDown if self.screen == Screen::Home && !self.help => self.cursor.0 = (self.cursor.0 + 1).min(self.cats.len() - 1),
            _ => {}
        }
    }

    /// Where the explorer is walking to.
    fn walker_goal(&self) -> f32 {
        match (&self.playing, self.phase) {
            (_, Phase::Crossing | Phase::Done { .. }) => PLANKS as f32 + 1.5,
            // Not onto a plank that is still in the air.
            (Some((_, _, round)), _) => (round.planks - usize::from(self.drop.as_ref().is_some_and(|d| d.good))) as f32,
            _ => 0.0,
        }
    }

    /// Whether something is moving that should be drawn smoothly.
    pub fn animating(&self) -> bool {
        self.animations
            && (self.screen == Screen::Intro
                || self.drop.is_some()
                || !self.particles.is_empty()
                || self.phase == Phase::Crossing
                || self.walker != self.walker_goal())
    }

    /// Lets time pass. Returns whether the screen should be drawn again.
    pub fn tick(&mut self) -> bool {
        let now = Instant::now();
        let passed = self.last_tick.map_or(0.0, |last| now.duration_since(last).as_secs_f32()).min(0.1);
        self.last_tick = Some(now);
        self.advance(passed)
    }

    pub fn advance(&mut self, dt: f32) -> bool {
        self.phase_time += dt;
        if !self.animations {
            // Nothing moves, so there is nothing new to draw until a key is pressed.
            self.walker = self.walker_goal();
            return false;
        }
        self.time += dt;

        if self.screen == Screen::Intro {
            if !self.intro_burst && self.phase_time >= INTRO_LETTER * 7.8 {
                self.intro_burst = true;
                self.confetti(70);
            }
            if self.phase_time >= INTRO {
                self.home();
            }
        }

        if let Some(drop) = &mut self.drop {
            drop.age += dt;
            if drop.age >= if drop.good { DROP } else { SINK } {
                self.drop = None;
            }
        }

        let goal = self.walker_goal();
        if self.walker != goal {
            let speed = if self.phase == Phase::Crossing { 3.5 } else { 5.0 };
            self.walker = if self.walker < goal { (self.walker + speed * dt).min(goal) } else { goal };
            if self.phase == Phase::Crossing {
                self.step_in -= dt;
                if self.step_in <= 0.0 {
                    self.step_in = 0.26;
                    self.play(Sound::Step);
                }
            }
        } else if self.phase == Phase::Crossing {
            self.finish();
        }

        if let Phase::Done { stars, .. } = self.phase {
            let due = ((self.phase_time / STAR_EVERY) as u8).min(stars);
            if due > self.stars_shown {
                self.stars_shown += 1;
                self.play(Sound::Star);
            }
        }

        let floor = self.size.1 as f32;
        for p in &mut self.particles {
            p.age += dt;
            p.vy += p.gravity * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
        }
        self.particles.retain(|p| p.age < p.life && p.y < floor);
        true
    }

    /// Confetti from the top of the window.
    fn confetti(&mut self, pieces: usize) {
        if !self.animations {
            return;
        }
        let theme = self.theme();
        let colors = [theme.answers[0], theme.answers[1], theme.answers[2], theme.answers[3], theme.accent];
        for _ in 0..pieces {
            let symbol = ["■", "●", "▲", "◆", "★"][self.rng.below(5)];
            let particle = Particle {
                x: self.rng.unit() * self.size.0 as f32,
                y: -self.rng.unit() * self.size.1 as f32 * 0.6,
                vx: self.rng.unit() * 6.0 - 3.0,
                vy: 5.0 + self.rng.unit() * 9.0,
                gravity: 3.0,
                age: 0.0,
                life: 6.0,
                color: colors[self.rng.below(colors.len())],
                symbol,
                front: false,
            };
            self.particles.push(particle);
        }
    }

    /// Sparkles flying out of a button.
    fn sparkle(&mut self, from: Rect) {
        let theme = self.theme();
        let (cx, cy) = (from.x as f32 + from.width as f32 / 2.0, from.y as f32 + from.height as f32 / 2.0);
        for i in 0..16 {
            let turn = i as f32 / 16.0 * std::f32::consts::TAU;
            let speed = 6.0 + self.rng.unit() * 10.0;
            // A cell is twice as tall as it is wide, so half the speed downwards.
            let particle = Particle {
                x: cx,
                y: cy,
                vx: turn.cos() * speed * 2.0,
                vy: turn.sin() * speed,
                gravity: 14.0,
                age: 0.0,
                life: 0.5 + self.rng.unit() * 0.4,
                color: if i % 2 == 0 { theme.sun } else { theme.accent },
                symbol: if i % 2 == 0 { "★" } else { "✦" },
                front: true,
            };
            self.particles.push(particle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quiz;

    fn app() -> App {
        let mut app = App::new(true, Settings::default(), quiz::builtin(), Vec::new());
        app.speaker = Speaker::silent();
        app
    }

    fn press(app: &mut App, code: KeyCode) {
        app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn round(app: &App) -> &Round {
        &app.playing.as_ref().unwrap().2
    }

    /// Answers right until the bridge is built, then lets the explorer cross.
    fn build_the_bridge(app: &mut App) {
        while !round(app).built() {
            let right = round(app).asked.correct;
            press(app, KeyCode::Char((b'a' + right as u8) as char));
            assert_eq!(app.phase, Phase::Answered { picked: right, right: true });
            if !round(app).built() {
                press(app, KeyCode::Enter);
            }
        }
        press(app, KeyCode::Enter);
        for _ in 0..200 {
            app.advance(0.05);
        }
    }

    #[test]
    fn mix_is_every_category_together() {
        let app = app();
        let mix = app.cats.last().unwrap();
        assert_eq!(mix.id, "mix");
        assert_eq!(mix.questions.len(), app.cats[..app.cats.len() - 1].iter().map(|c| c.questions.len()).sum::<usize>());
        assert_eq!(app.stars(), (0, app.cats.len() * 9));
        assert_eq!(app.rank(), "New Explorer");
    }

    #[test]
    fn a_round_from_the_passport_to_the_stars() {
        let mut app = app();
        assert_eq!(app.screen, Screen::Home);
        // Down one category and right to Medium, with the keys.
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Right);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.screen, Screen::Play);
        let (cat, level, _) = app.playing.as_ref().unwrap();
        assert_eq!((*cat, *level), (1, Level::Medium));
        assert!(round(&app).asked.question.level == Level::Medium);

        // A wrong answer: no plank, a plank in the river, and on with Enter.
        let wrong = (round(&app).asked.correct + 1) % 4;
        press(&mut app, KeyCode::Char((b'1' + wrong as u8) as char));
        assert_eq!(app.phase, Phase::Answered { picked: wrong, right: false });
        assert!(app.drop.as_ref().is_some_and(|d| !d.good && d.slot == 0));
        assert_eq!((round(&app).planks, round(&app).mistakes), (0, 1));
        // A second answer to the same question does not count.
        press(&mut app, KeyCode::Char('a'));
        assert_eq!(round(&app).mistakes, 1);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.phase, Phase::Asking);

        // The marker and Enter answer too.
        let right = round(&app).asked.correct;
        app.focus = right;
        press(&mut app, KeyCode::Char(' '));
        assert_eq!(round(&app).planks, 1);
        // The explorer waits for the plank to land, then steps onto it.
        assert_eq!(app.walker_goal(), 0.0);
        for _ in 0..20 {
            app.advance(0.05);
        }
        assert!(app.drop.is_none());
        assert_eq!(app.walker, 1.0);
        press(&mut app, KeyCode::Enter);

        build_the_bridge(&mut app);
        assert_eq!(app.phase, Phase::Done { stars: 3, best: true });
        assert_eq!(app.stars_shown, 3);
        assert_eq!(app.progress.stars(&app.cats[1].id, Level::Medium), 3);
        assert_eq!(app.rank(), "Pathfinder");

        // Play again from the end, by its letter; then back to the passport.
        press(&mut app, KeyCode::Char('p'));
        assert_eq!((app.phase, round(&app).planks, app.walker), (Phase::Asking, 0, 0.0));
        press(&mut app, KeyCode::Esc);
        assert_eq!((app.screen, app.cursor), (Screen::Home, (1, 1)));
        assert!(app.playing.is_none() && !app.quit);
        // On the passport Esc does nothing; only Q quits.
        press(&mut app, KeyCode::Esc);
        assert!(!app.quit && app.screen == Screen::Home);
        press(&mut app, KeyCode::Char('q'));
        assert!(app.quit);
    }
    #[test]
    fn the_second_round_brings_new_questions() {
        let mut app = app();
        let first: Vec<Question> = app.deal(0, Level::Easy);
        let second: Vec<Question> = app.deal(0, Level::Easy);
        assert_eq!((first.len(), second.len()), (HAND, HAND));
        let total = app.cats[0].count(Level::Easy);
        let shared = second.iter().filter(|q| first.contains(q)).count();
        // Only as many are seen again as the category is short of two full hands.
        assert_eq!(shared, (2 * HAND).saturating_sub(total));
        assert!((1..HAND).all(|i| !second[..i].contains(&second[i])));
    }

    #[test]
    fn worse_stars_do_not_replace_better_ones() {
        let mut progress = Progress::default();
        assert!(progress.earn("flags", Level::Hard, 2));
        assert!(!progress.earn("flags", Level::Hard, 1));
        assert!(progress.earn("flags", Level::Hard, 3));
        assert!(!progress.earn("flags", Level::Hard, 3));
        progress.earn("nature", Level::Easy, 1);
        assert_eq!((progress.stars("flags", Level::Hard), progress.stars("nature", Level::Easy), progress.stars("nature", Level::Hard)), (3, 1, 0));
    }

    #[test]
    fn the_mouse_does_everything_the_keys_do() {
        let mut app = app();
        let click = |app: &mut App, kind: MouseEventKind, x: u16, y: u16| app.on_mouse(MouseEvent { kind, column: x, row: y, modifiers: KeyModifiers::NONE });
        app.buttons = vec![(Rect::new(0, 0, 10, 3), Action::Start(2, Level::Hard)), (Rect::new(20, 0, 5, 1), Action::Theme)];
        // The marker follows the pointer; a click on nothing does nothing.
        click(&mut app, MouseEventKind::Moved, 3, 1);
        assert_eq!(app.cursor, (2, 2));
        click(&mut app, MouseEventKind::Down(MouseButton::Left), 15, 1);
        click(&mut app, MouseEventKind::Down(MouseButton::Right), 3, 1);
        assert_eq!(app.screen, Screen::Home);
        click(&mut app, MouseEventKind::ScrollUp, 0, 0);
        assert_eq!(app.cursor, (1, 2));
        click(&mut app, MouseEventKind::Down(MouseButton::Left), 21, 0);
        assert_eq!(app.theme, 1);
        click(&mut app, MouseEventKind::Down(MouseButton::Left), 3, 1);
        assert_eq!(app.screen, Screen::Play);

        let right = round(&app).asked.correct;
        app.buttons = (0..4).map(|i| (Rect::new(i * 10, 5, 9, 3), Action::Answer(i as usize))).collect();
        app.buttons.push((Rect::new(0, 10, 9, 3), Action::Next));
        click(&mut app, MouseEventKind::Moved, 31, 6);
        assert_eq!(app.focus, 3);
        // Next does nothing before an answer.
        click(&mut app, MouseEventKind::Down(MouseButton::Left), 1, 11);
        assert_eq!(app.phase, Phase::Asking);
        click(&mut app, MouseEventKind::Down(MouseButton::Left), right as u16 * 10 + 1, 6);
        assert_eq!(app.phase, Phase::Answered { picked: right, right: true });
        assert!(!app.particles.is_empty(), "sparkles from the button");
        click(&mut app, MouseEventKind::Down(MouseButton::Left), 1, 11);
        assert_eq!(app.phase, Phase::Asking);

        // Letting go of the button is not a second click, but a release that no press
        // came before is one.
        let before = round(&app).asked.question.clone();
        let right_x = round(&app).asked.correct as u16 * 10 + 1;
        click(&mut app, MouseEventKind::Up(MouseButton::Left), right_x, 6);
        assert_eq!(app.phase, Phase::Asking);
        click(&mut app, MouseEventKind::Up(MouseButton::Left), right_x, 6);
        assert!(matches!(app.phase, Phase::Answered { right: true, .. }) && round(&app).asked.question == before);
        click(&mut app, MouseEventKind::Down(MouseButton::Left), 1, 11);
        click(&mut app, MouseEventKind::Up(MouseButton::Left), 1, 11);
        assert_eq!(app.phase, Phase::Asking);

        // Help closes on any click, and that click does nothing else.
        app.help = true;
        click(&mut app, MouseEventKind::Down(MouseButton::Left), 1, 6);
        assert!(!app.help && app.phase == Phase::Asking);
    }

    #[test]
    fn without_animations_nothing_waits() {
        let dir = std::env::temp_dir().join(format!("fungeo-motion-test-{}", std::process::id()));
        let mut app = app();
        app.settings_path = Some(dir.join("settings"));
        app.begin();
        assert_eq!(app.screen, Screen::Intro);
        // M in the intro switches the moving off, and with it the intro.
        press(&mut app, KeyCode::Char('x'));
        assert_eq!(app.screen, Screen::Home);
        press(&mut app, KeyCode::Char('m'));
        assert!(!app.animations && !Settings::load(&dir.join("settings")).animations);
        app.begin();
        assert_eq!(app.screen, Screen::Home);

        press(&mut app, KeyCode::Enter);
        let right = round(&app).asked.correct;
        press(&mut app, KeyCode::Char((b'a' + right as u8) as char));
        assert!(app.drop.is_none() && app.particles.is_empty());
        assert!(!app.advance(0.05) && !app.animating());
        assert_eq!(app.walker, 1.0);
        press(&mut app, KeyCode::Enter);
        // The last Enter goes straight to the stars.
        while !round(&app).built() {
            let right = round(&app).asked.correct;
            press(&mut app, KeyCode::Char((b'a' + right as u8) as char));
            press(&mut app, KeyCode::Enter);
        }
        assert_eq!((app.phase, app.stars_shown), (Phase::Done { stars: 3, best: true }, 3));
        assert!(app.particles.is_empty());

        press(&mut app, KeyCode::Char('s'));
        press(&mut app, KeyCode::Char('t'));
        let saved = Settings::load(&dir.join("settings"));
        assert_eq!((saved.sound, saved.theme, saved.update), (false, 1, true));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_intro_ends_by_itself_or_at_any_key() {
        let mut app = app();
        app.begin();
        assert!(app.animating());
        for _ in 0..(INTRO / 0.05) as usize + 2 {
            app.advance(0.05);
        }
        assert_eq!(app.screen, Screen::Home);
        // The confetti it threw is cleared with it.
        assert!(app.particles.is_empty());
        app.begin();
        press(&mut app, KeyCode::Char('q'));
        assert!(app.screen == Screen::Home && !app.quit);
    }

    #[test]
    fn the_explorer_crosses_with_footsteps_and_the_stars_come_one_by_one() {
        let mut app = app();
        app.speaker = Speaker::silent();
        app.start(0, Level::Easy);
        while !round(&app).built() {
            let right = round(&app).asked.correct;
            app.act(Action::Answer(right));
            if !round(&app).built() {
                app.act(Action::Next);
            }
        }
        for _ in 0..40 {
            app.advance(0.05);
        }
        assert_eq!(app.walker, PLANKS as f32);
        app.act(Action::Next);
        assert_eq!(app.phase, Phase::Crossing);
        app.advance(0.05);
        assert!(app.walker > PLANKS as f32 && app.animating());
        for _ in 0..12 {
            app.advance(0.05);
        }
        assert!(matches!(app.phase, Phase::Done { stars: 3, .. }));
        assert_eq!(app.stars_shown, 0);
        app.advance(STAR_EVERY);
        assert_eq!(app.stars_shown, 1);
        for _ in 0..4 {
            app.advance(STAR_EVERY);
        }
        assert_eq!(app.stars_shown, 3);
        let heard = &app.speaker.heard;
        assert_eq!(heard.iter().filter(|&&s| s == Sound::Right).count(), PLANKS);
        assert!(heard.contains(&Sound::Step) && heard.contains(&Sound::Win));
        assert_eq!(heard.iter().filter(|&&s| s == Sound::Star).count(), 3);
        // Enter on the marker's button: back to the passport.
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.screen, Screen::Home);
    }
}
