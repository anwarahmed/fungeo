//! The questions: the text format they are written in, the categories they belong to,
//! flags as shapes, and a round of the game. Knows nothing about the screen.
//!
//! A category is one text file. The ones in `questions/` are built into the program
//! (`build.rs` lists them); a player's own, in the configuration directory, are read
//! when the game starts. Nothing else has to change to add one.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use crate::theme::Rgb;

/// Right answers that finish a bridge.
pub const PLANKS: usize = 8;
/// A level of a category can be played once it has this many questions.
pub const MIN_QUESTIONS: usize = 4;
/// Longer texts than these do not fit the screen well.
const MAX_QUESTION: usize = 90;
const MAX_ANSWER: usize = 28;
const MAX_FACT: usize = 120;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Level {
    Easy,
    Medium,
    Hard,
}

impl Level {
    pub const ALL: [Level; 3] = [Level::Easy, Level::Medium, Level::Hard];

    pub fn name(self) -> &'static str {
        match self {
            Level::Easy => "Easy",
            Level::Medium => "Medium",
            Level::Hard => "Hard",
        }
    }

    pub fn parse(name: &str) -> Option<Level> {
        Level::ALL.into_iter().find(|l| l.name().eq_ignore_ascii_case(name))
    }
}

/// Stripes: each a color and how many parts of the whole it takes.
type Bands = Vec<(Rgb, u32)>;

/// A flag that is only simple shapes, which is all a terminal can draw well.
#[derive(Clone, PartialEq, Debug)]
pub enum Flag {
    /// Stripes side by side (`vertical`) or one above the other.
    Stripes { vertical: bool, bands: Bands },
    /// A cross lying on its side, as in Denmark's flag, with a second cross inside it
    /// in Norway's and Iceland's.
    Nordic { field: Rgb, cross: Rgb, inner: Option<Rgb> },
    /// A circle, in the middle or a little towards the pole.
    Disc { field: Rgb, disc: Rgb, left: bool },
    /// Switzerland's: a short cross in the middle of a square.
    Plus { field: Rgb, plus: Rgb },
    /// A bar down the pole side, this many hundredths wide, and stripes beside it.
    Bar { bar: Rgb, width: u32, bands: Bands },
    /// Any of the above with marks drawn over it, the later over the earlier.
    Layered { base: Box<Flag>, marks: Vec<Mark> },
}

/// Something drawn over a flag. Sizes are parts of the flag's height and places parts
/// of its width and height, all from 0 to 1.
#[derive(Clone, PartialEq, Debug)]
pub enum Mark {
    Disc {
        color: Rgb,
        size: f32,
        at: (f32, f32),
    },
    /// A star of five points, one of them up.
    Star {
        color: Rgb,
        size: f32,
        at: (f32, f32),
    },
    /// A triangle with one side along the pole, reaching this far across.
    Triangle {
        color: Rgb,
        reach: f32,
    },
}

impl Mark {
    /// `disc COLOR [SIZE [X Y]]`, `star COLOR [SIZE [X Y]]` or `triangle COLOR [REACH]`,
    /// the numbers in hundredths; a disc or star is in the middle unless it says.
    fn parse(spec: &str) -> Result<Mark, String> {
        let words: Vec<&str> = spec.split_whitespace().collect();
        let part = |word: &str| {
            word.parse::<u32>().ok().filter(|n| (1..100).contains(n)).map(|n| n as f32 / 100.0).ok_or(format!("'{word}' is not a number from 1 to 99"))
        };
        let (size, at) = match words.get(2..).unwrap_or(&[]) {
            [] => (0.2, (0.5, 0.5)),
            [size] => (part(size)?, (0.5, 0.5)),
            [size, x, y] => (part(size)?, (part(x)?, part(y)?)),
            _ => return Err("after the color come SIZE, or SIZE X Y".into()),
        };
        match words.as_slice() {
            ["disc", name, ..] => Ok(Mark::Disc { color: color(name)?, size, at }),
            ["star", name, ..] => Ok(Mark::Star { color: color(name)?, size, at }),
            ["triangle", name] => Ok(Mark::Triangle { color: color(name)?, reach: 0.4 }),
            ["triangle", name, reach] => Ok(Mark::Triangle { color: color(name)?, reach: part(reach)? }),
            _ => Err("after + comes one of: disc COLOR [SIZE [X Y]], star COLOR [SIZE [X Y]], triangle COLOR [REACH]".into()),
        }
    }

    /// Its color, where it covers this point of the flag.
    fn at(&self, x: f32, y: f32) -> Option<Rgb> {
        match *self {
            Mark::Disc { color, size, at } => {
                let (dx, dy) = ((x - at.0) * 1.5, y - at.1);
                (dx * dx + dy * dy < size * size).then_some(color)
            }
            Mark::Star { color, size, at } => {
                let (dx, dy) = ((x - at.0) * 1.5, y - at.1);
                // Folded into one tenth of the star: between a point and the notch
                // beside it, where the edge is a single straight line.
                let fifth = std::f32::consts::TAU / 5.0;
                let turn = dx.atan2(-dy).rem_euclid(fifth);
                let turn = turn.min(fifth - turn);
                let far = dx.hypot(dy);
                let (px, py) = (far * turn.cos(), far * turn.sin());
                let notch = (size * 0.382 * (fifth / 2.0).cos(), size * 0.382 * (fifth / 2.0).sin());
                let side = |x: f32, y: f32| (notch.0 - size) * y - notch.1 * (x - size);
                (side(px, py) * side(0.0, 0.0) >= 0.0).then_some(color)
            }
            Mark::Triangle { color, reach } => (x < reach * (1.0 - (y - 0.5).abs() * 2.0)).then_some(color),
        }
    }
}

fn color(name: &str) -> Result<Rgb, String> {
    Ok(match name {
        "red" => (206, 17, 38),
        "white" => (255, 255, 255),
        "blue" => (0, 70, 173),
        "navy" => (0, 32, 91),
        "lightblue" => (108, 172, 228),
        "green" => (0, 146, 70),
        "yellow" => (255, 205, 0),
        "orange" => (255, 130, 0),
        "black" => (20, 20, 20),
        "maroon" => (158, 48, 57),
        _ => {
            let hex = name.strip_prefix('#').filter(|h| h.len() == 6 && h.is_ascii()).ok_or(format!("unknown color '{name}'"))?;
            let part = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| format!("unknown color '{name}'"));
            (part(0)?, part(2)?, part(4)?)
        }
    })
}

/// `red white*2 blue`: colors, each taking one part unless it says more.
fn bands(words: &[&str]) -> Result<Bands, String> {
    if words.len() < 2 {
        return Err("a flag needs at least two stripes".into());
    }
    words
        .iter()
        .map(|word| match word.split_once('*') {
            Some((name, parts)) => Ok((color(name)?, parts.parse().ok().filter(|&p| p > 0).ok_or(format!("'{word}' needs a number after *"))?)),
            None => Ok((color(word)?, 1)),
        })
        .collect()
}

fn band_at(bands: &Bands, at: f32) -> Rgb {
    let total: u32 = bands.iter().map(|b| b.1).sum();
    let mut end = 0;
    for &(color, parts) in bands {
        end += parts;
        if at * (total as f32) < end as f32 {
            return color;
        }
    }
    bands[bands.len() - 1].0
}

impl Flag {
    /// From the words after `P: flag`: a flag, and then any marks over it, each after
    /// a `+`, as in `h red red + star yellow 30`.
    pub fn parse(spec: &str) -> Result<Flag, String> {
        let mut parts = spec.split('+');
        let base = Flag::plain(parts.next().unwrap_or(""))?;
        let marks = parts.map(Mark::parse).collect::<Result<Vec<Mark>, String>>()?;
        Ok(if marks.is_empty() { base } else { Flag::Layered { base: Box::new(base), marks } })
    }

    fn plain(spec: &str) -> Result<Flag, String> {
        let words: Vec<&str> = spec.split_whitespace().collect();
        match words.as_slice() {
            ["h", rest @ ..] => Ok(Flag::Stripes { vertical: false, bands: bands(rest)? }),
            ["v", rest @ ..] => Ok(Flag::Stripes { vertical: true, bands: bands(rest)? }),
            ["nordic", field, cross] => Ok(Flag::Nordic { field: color(field)?, cross: color(cross)?, inner: None }),
            ["nordic", field, cross, inner] => Ok(Flag::Nordic { field: color(field)?, cross: color(cross)?, inner: Some(color(inner)?) }),
            ["disc", field, disc] => Ok(Flag::Disc { field: color(field)?, disc: color(disc)?, left: false }),
            ["disc", field, disc, "left"] => Ok(Flag::Disc { field: color(field)?, disc: color(disc)?, left: true }),
            ["plus", field, plus] => Ok(Flag::Plus { field: color(field)?, plus: color(plus)? }),
            ["bar", bar, width, rest @ ..] => Ok(Flag::Bar {
                bar: color(bar)?,
                width: width.parse().ok().filter(|w| (1..100).contains(w)).ok_or("the bar's width is a number from 1 to 99")?,
                bands: bands(rest)?,
            }),
            _ => {
                Err("a flag is one of: h COLORS, v COLORS, nordic FIELD CROSS [INNER], disc FIELD DISC [left], plus FIELD PLUS, bar COLOR WIDTH COLORS".into())
            }
        }
    }

    /// Whether it is square, as Switzerland's is, and not three wide to two tall.
    pub fn square(&self) -> bool {
        match self {
            Flag::Layered { base, .. } => base.square(),
            _ => matches!(self, Flag::Plus { .. }),
        }
    }

    /// The color at a point of the flag, both from 0 to 1, from the top by the pole.
    pub fn at(&self, x: f32, y: f32) -> Rgb {
        match self {
            Flag::Layered { base, marks } => marks.iter().rev().find_map(|m| m.at(x, y)).unwrap_or_else(|| base.at(x, y)),
            Flag::Stripes { vertical, bands } => band_at(bands, if *vertical { x } else { y }),
            Flag::Nordic { field, cross, inner } => {
                // The upright is nearer the pole; the measures are Denmark's and Norway's.
                let (wide, narrow) = if inner.is_some() { (0.09, 0.045) } else { (0.055, 0.0) };
                let (dx, dy) = ((x - 0.36).abs(), (y - 0.5).abs() * 2.0 / 3.0);
                match inner {
                    Some(inner) if dx < narrow || dy < narrow => *inner,
                    _ if dx < wide || dy < wide => *cross,
                    _ => *field,
                }
            }
            Flag::Disc { field, disc, left } => {
                let (dx, dy) = ((x - if *left { 0.45 } else { 0.5 }) * 1.5, y - 0.5);
                if dx * dx + dy * dy < 0.31 * 0.31 { *disc } else { *field }
            }
            Flag::Plus { field, plus } => {
                let (dx, dy) = ((x - 0.5).abs(), (y - 0.5).abs());
                if (dx < 0.1 && dy < 0.32) || (dy < 0.1 && dx < 0.32) { *plus } else { *field }
            }
            Flag::Bar { bar, width, bands } => {
                if x * 100.0 < *width as f32 {
                    *bar
                } else {
                    band_at(bands, y)
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Question {
    pub level: Level,
    pub text: String,
    pub answer: String,
    /// At least three wrong answers; three of them are shown.
    pub wrong: Vec<String>,
    /// Something worth knowing, shown once the question is answered. May be empty.
    pub fact: String,
    pub picture: Option<Flag>,
}

#[derive(Clone, Debug)]
pub struct Category {
    /// The file's name without `.txt`; progress is kept under it.
    pub id: String,
    pub name: String,
    pub color: Rgb,
    /// Where it comes in the list; lower is earlier.
    pub order: i32,
    pub questions: Vec<Question>,
}

/// A question as it is being read, before its wrong answers are settled.
#[derive(Default)]
struct Draft {
    line: usize,
    text: Option<String>,
    answer: Option<String>,
    wrong: Vec<String>,
    fact: String,
    picture: Option<Flag>,
}

impl Category {
    /// Reads a category from the text of its file. An error names the line.
    ///
    /// ```text
    /// name: Capitals
    /// color: #e85d75
    /// order: 1
    /// ask: A question for every entry that has no Q of its own
    ///
    /// [easy]
    /// Q: What is the capital of France?
    /// A: Paris
    /// X: London | Rome | Madrid
    /// F: A fact to show afterwards.
    /// P: flag v blue white red
    /// ```
    ///
    /// Blank lines separate questions. Without `X`, the wrong answers are the right
    /// answers of the category's other such questions, from the same level first.
    pub fn parse(id: &str, source: &str) -> Result<Category, String> {
        let mut category = Category { id: id.to_string(), name: id.to_string(), color: (120, 144, 220), order: 100, questions: Vec::new() };
        let mut ask: Option<String> = None;
        let mut level: Option<Level> = None;
        let mut drafts: Vec<(Level, Draft)> = Vec::new();
        let mut draft = Draft::default();

        let mut finish = |draft: &mut Draft, level: Option<Level>| -> Result<(), String> {
            let done = std::mem::take(draft);
            if done.line == 0 {
                return Ok(());
            }
            let level = level.ok_or(format!("line {}: a question before the first [easy], [medium] or [hard]", done.line))?;
            drafts.push((level, done));
            Ok(())
        };

        for (number, line) in source.lines().enumerate().map(|(i, l)| (i + 1, l.trim())) {
            let fail = |what: &str| format!("line {number}: {what}");
            if line.is_empty() {
                finish(&mut draft, level)?;
            } else if line.starts_with('#') {
            } else if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                finish(&mut draft, level)?;
                level = Some(Level::parse(name.trim()).ok_or(fail("the levels are [easy], [medium] and [hard]"))?);
            } else {
                let (key, value) = line.split_once(':').map(|(k, v)| (k.trim(), v.trim())).ok_or(fail("expected 'KEY: text'"))?;
                if value.is_empty() {
                    return Err(fail("nothing after the colon"));
                }
                if draft.line == 0 && matches!(key, "Q" | "A" | "X" | "F" | "P") {
                    draft.line = number;
                }
                match key {
                    "Q" => draft.text = Some(value.to_string()),
                    "A" => draft.answer = Some(value.to_string()),
                    "X" => draft.wrong = value.split('|').map(|w| w.trim().to_string()).filter(|w| !w.is_empty()).collect(),
                    "F" => draft.fact = value.to_string(),
                    "P" => {
                        let spec = value.strip_prefix("flag ").ok_or(fail("the only picture is 'flag ...'"))?;
                        draft.picture = Some(Flag::parse(spec).map_err(|e| fail(&e))?);
                    }
                    "name" if level.is_none() => category.name = value.to_string(),
                    "color" if level.is_none() => category.color = color(value).map_err(|e| fail(&e))?,
                    "order" if level.is_none() => category.order = value.parse().map_err(|_| fail("the order is a whole number"))?,
                    "ask" if level.is_none() => ask = Some(value.to_string()),
                    _ => return Err(fail(&format!("'{key}' is not known here"))),
                }
            }
        }
        finish(&mut draft, level)?;

        // The right answers that questions without wrong ones of their own share.
        let shared: Vec<(Level, String)> = drafts.iter().filter(|(_, d)| d.wrong.is_empty()).filter_map(|(l, d)| Some((*l, d.answer.clone()?))).collect();
        for (level, draft) in drafts {
            let fail = |what: String| format!("line {}: {what}", draft.line);
            let answer = draft.answer.ok_or(fail("a question needs its answer, 'A: ...'".into()))?;
            let text = draft.text.or(ask.clone()).ok_or(fail("a question needs 'Q: ...', or the file an 'ask: ...' line".into()))?;
            let mut wrong = draft.wrong;
            if wrong.is_empty() {
                for same_level in [true, false] {
                    for (other, candidate) in &shared {
                        if (*other == level) == same_level && *candidate != answer && !wrong.contains(candidate) && (same_level || wrong.len() < 3) {
                            wrong.push(candidate.clone());
                        }
                    }
                }
            }
            if wrong.len() < 3 {
                return Err(fail(format!("'{answer}' needs three wrong answers, 'X: one | two | three'")));
            }
            if wrong.contains(&answer) || (1..wrong.len()).any(|i| wrong[..i].contains(&wrong[i])) {
                return Err(fail(format!("the wrong answers of '{answer}' repeat one another or the right one")));
            }
            for (what, value, most) in [("question", &text, MAX_QUESTION), ("fact", &draft.fact, MAX_FACT)] {
                if value.chars().count() > most {
                    return Err(fail(format!("the {what} is longer than {most} characters")));
                }
            }
            if let Some(long) = wrong.iter().chain([&answer]).find(|a| a.chars().count() > MAX_ANSWER) {
                return Err(fail(format!("the answer '{long}' is longer than {MAX_ANSWER} characters")));
            }
            category.questions.push(Question { level, text, answer, wrong, fact: draft.fact, picture: draft.picture });
        }
        if category.questions.is_empty() {
            return Err("there are no questions".into());
        }
        Ok(category)
    }

    pub fn count(&self, level: Level) -> usize {
        self.questions.iter().filter(|q| q.level == level).count()
    }

    /// Whether there are enough questions at this level to play it.
    pub fn has(&self, level: Level) -> bool {
        self.count(level) >= MIN_QUESTIONS
    }
}

include!(concat!(env!("OUT_DIR"), "/builtin.rs"));

/// The categories built into the program.
pub fn builtin() -> Vec<Category> {
    BUILTIN.iter().map(|(id, source)| Category::parse(id, source).unwrap_or_else(|e| panic!("questions/{id}.txt: {e}"))).collect()
}

/// Where a player's own categories are: `questions` in `~/.config/fungeo`, unless
/// `XDG_CONFIG_HOME` says otherwise.
pub fn own_dir() -> PathBuf {
    match std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"),
    }
    .join("fungeo/questions")
}

/// Reads one category file; its name without `.txt` is the category's id.
pub fn read(path: &Path) -> Result<Category, String> {
    let id = path.file_stem().and_then(|s| s.to_str()).filter(|s| !s.is_empty() && *s != "mix").ok_or("the file needs a name, and not 'mix'")?;
    let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    Category::parse(id, &source)
}

/// Every category: the built-in ones, then those in `dir`. A file there with the name
/// of a built-in one takes its place. Files that cannot be read are left out and
/// named in the second list.
pub fn load(dir: &Path) -> (Vec<Category>, Vec<String>) {
    let mut categories = builtin();
    let mut problems = Vec::new();
    let mut files: Vec<PathBuf> =
        std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "txt")).collect();
    files.sort();
    for file in files {
        match read(&file) {
            Ok(category) => {
                categories.retain(|c| c.id != category.id);
                categories.push(category);
            }
            Err(e) => problems.push(format!("{}: {e}", file.display())),
        }
    }
    categories.sort_by_key(|c| c.order);
    (categories, problems)
}

/// Small and good enough for shuffling questions.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed | 1)
    }

    pub fn from_clock() -> Rng {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
        Rng::new(now ^ (std::process::id() as u64) << 32)
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A whole number from 0 up to, not including, `n`.
    pub fn below(&mut self, n: usize) -> usize {
        ((self.next() >> 33) as usize) % n.max(1)
    }

    /// From 0 up to 1.
    pub fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            items.swap(i, self.below(i + 1));
        }
    }
}

/// A question as it is put to the player.
#[derive(Clone, Debug)]
pub struct Asked {
    pub question: Question,
    /// The right answer and three wrong ones, shuffled.
    pub options: Vec<String>,
    /// Which of them is right.
    pub correct: usize,
}

impl Asked {
    fn new(question: Question, rng: &mut Rng) -> Asked {
        let mut wrong = question.wrong.clone();
        rng.shuffle(&mut wrong);
        let mut options: Vec<String> = wrong.into_iter().take(3).collect();
        let correct = rng.below(options.len() + 1);
        options.insert(correct, question.answer.clone());
        Asked { question, options, correct }
    }
}

/// One bridge: questions until `PLANKS` of them have been answered right.
pub struct Round {
    pub planks: usize,
    pub mistakes: usize,
    pub asked: Asked,
    /// What is asked next. A question answered wrong goes to the back, to be met again.
    queue: VecDeque<Question>,
    /// Everything the round started with, for when the queue runs out.
    all: Vec<Question>,
}

impl Round {
    /// `questions` in the order to ask them; there must be at least one.
    pub fn new(questions: Vec<Question>, rng: &mut Rng) -> Round {
        let mut queue: VecDeque<Question> = questions.iter().cloned().collect();
        let first = queue.pop_front().expect("a round needs questions");
        Round { planks: 0, mistakes: 0, asked: Asked::new(first, rng), queue, all: questions }
    }

    /// Takes the player's answer and says whether it was right.
    pub fn answer(&mut self, option: usize) -> bool {
        let right = option == self.asked.correct;
        if right {
            self.planks += 1;
        } else {
            self.mistakes += 1;
            self.queue.push_back(self.asked.question.clone());
        }
        right
    }

    pub fn built(&self) -> bool {
        self.planks >= PLANKS
    }

    pub fn next(&mut self, rng: &mut Rng) {
        if self.queue.is_empty() {
            let mut again = self.all.clone();
            rng.shuffle(&mut again);
            // Not the same question twice running, when there is another.
            if again.len() > 1 && again[0] == self.asked.question {
                again.swap(0, 1);
            }
            self.queue = again.into();
        }
        let question = self.queue.pop_front().expect("just filled");
        self.asked = Asked::new(question, rng);
    }

    /// Three stars for a bridge built with no mistake or one, two for up to three
    /// mistakes, and one for finishing at all: nobody leaves with nothing.
    pub fn stars(&self) -> u8 {
        match self.mistakes {
            0 | 1 => 3,
            2 | 3 => 2,
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# A comment.
name: Sample
color: #102030
order: 7
ask: Which one?

[easy]
Q: What is the capital of France?
A: Paris
X: London | Rome | Madrid
F: A fact.

A: One
P: flag v blue white red
";

    const MORE: &str = "\nA: Two\n\nA: Three\n\n[hard]\nA: Four\n";

    #[test]
    fn a_file_is_read_into_a_category() {
        let category = Category::parse("sample", &format!("{SAMPLE}{MORE}")).unwrap();
        assert_eq!((category.name.as_str(), category.color, category.order), ("Sample", (16, 32, 48), 7));
        assert_eq!((category.count(Level::Easy), category.count(Level::Medium), category.count(Level::Hard)), (4, 0, 1));
        assert!(category.has(Level::Easy) && !category.has(Level::Hard));
        let paris = &category.questions[0];
        assert_eq!((paris.text.as_str(), paris.answer.as_str(), paris.fact.as_str()), ("What is the capital of France?", "Paris", "A fact."));
        assert_eq!(paris.wrong, ["London", "Rome", "Madrid"]);
        let one = &category.questions[1];
        assert_eq!((one.text.as_str(), one.wrong.as_slice()), ("Which one?", &["Two".to_string(), "Three".into(), "Four".into()][..]));
        assert!(one.picture.is_some());
        // The hard one has no others at its level, so it borrows from the easy ones.
        assert_eq!(category.questions[4].wrong, ["One", "Two", "Three"]);
    }

    #[test]
    fn mistakes_in_a_file_are_named_by_line() {
        let check = |source: &str, expected: &str| {
            let error = Category::parse("x", source).unwrap_err();
            assert!(error.contains(expected), "{error:?} does not mention {expected:?}");
        };
        check("", "no questions");
        check("Q: a?\nA: b\nX: c | d | e", "line 1: a question before");
        check("[tiny]", "line 1: the levels");
        check("[easy]\nwhat", "line 2: expected");
        check("[easy]\nQ:", "line 2: nothing after");
        check("[easy]\nname: late", "line 2: 'name' is not known here");
        check("[easy]\nQ: a?", "line 2: a question needs its answer");
        check("[easy]\nA: b\nX: c | d | e", "line 2: a question needs 'Q");
        check("[easy]\nQ: a?\nA: b\nX: c | d", "line 2: 'b' needs three wrong");
        check("[easy]\nQ: a?\nA: b\nX: c | d | b", "repeat");
        check("[easy]\nQ: a?\nA: b\nX: c | d | d", "repeat");
        check("[easy]\nQ: a?\nA: b\nX: c | d | e\nP: photo x", "line 5: the only picture");
        check("[easy]\nQ: a?\nA: b\nX: c | d | e\nP: flag h red", "line 5: a flag needs at least two");
        check("[easy]\nQ: a?\nA: b\nX: c | d | e\nP: flag h red pink", "line 5: unknown color 'pink'");
        check("color: #12345", "line 1: unknown color");
        check(&format!("[easy]\nQ: a?\nA: b\nX: c | d | {}", "e".repeat(29)), "longer than 28");
        check(&format!("[easy]\nQ: {}\nA: b\nX: c | d | e", "a".repeat(91)), "the question is longer");
    }

    #[test]
    fn flags_are_shapes() {
        let (red, white, blue) = (color("red").unwrap(), color("white").unwrap(), color("blue").unwrap());
        let france = Flag::parse("v blue white red").unwrap();
        assert_eq!((france.at(0.1, 0.5), france.at(0.5, 0.5), france.at(0.9, 0.5)), (blue, white, red));
        let colombia = Flag::parse("h yellow*2 blue red").unwrap();
        assert_eq!((colombia.at(0.5, 0.45), colombia.at(0.5, 0.6), colombia.at(0.5, 0.99)), (color("yellow").unwrap(), blue, red));
        let japan = Flag::parse("disc white red").unwrap();
        assert_eq!((japan.at(0.5, 0.5), japan.at(0.05, 0.05), japan.at(0.5, 0.1)), (red, white, white));
        let norway = Flag::parse("nordic red white navy").unwrap();
        assert_eq!(
            (norway.at(0.36, 0.1), norway.at(0.9, 0.5), norway.at(0.9, 0.6), norway.at(0.9, 0.9)),
            (color("navy").unwrap(), color("navy").unwrap(), white, red)
        );
        let swiss = Flag::parse("plus red white").unwrap();
        assert!(swiss.square() && !japan.square());
        assert_eq!((swiss.at(0.5, 0.25), swiss.at(0.25, 0.25)), (white, red));
        let emirates = Flag::parse("bar red 25 green white black").unwrap();
        assert_eq!((emirates.at(0.1, 0.9), emirates.at(0.5, 0.1), emirates.at(0.5, 0.9)), (red, color("green").unwrap(), color("black").unwrap()));
        // Marks over a flag: Vietnam's star, the Czech triangle, Cuba's star on its triangle.
        let yellow = color("yellow").unwrap();
        let vietnam = Flag::parse("h red red + star yellow 30").unwrap();
        // The middle and the top point are star; between two points, and outside, are not.
        assert_eq!((vietnam.at(0.5, 0.5), vietnam.at(0.5, 0.25), vietnam.at(0.5, 0.75), vietnam.at(0.9, 0.5)), (yellow, yellow, red, red));
        assert_eq!(vietnam.at(0.5 + 0.25 / 1.5, 0.42), yellow, "the point to the right");
        let czech = Flag::parse("h white red + triangle blue 50").unwrap();
        assert_eq!((czech.at(0.2, 0.5), czech.at(0.45, 0.5), czech.at(0.2, 0.1), czech.at(0.2, 0.9), czech.at(0.8, 0.9)), (blue, blue, white, red, red));
        let cuba = Flag::parse("h blue white blue + triangle red 40 + star white 12 14 50").unwrap();
        assert_eq!((cuba.at(0.14, 0.5), cuba.at(0.3, 0.5), cuba.at(0.9, 0.5)), (white, red, white));
        let laos = Flag::parse("h red blue*2 red + disc white").unwrap();
        assert_eq!((laos.at(0.5, 0.5), laos.at(0.5, 0.72), laos.at(0.5, 0.9)), (white, blue, red));
        assert!(Flag::parse("h red white + moon white").is_err() && Flag::parse("h red white + star white 200").is_err());
        assert!(Flag::parse("h red white + disc white 10 20").is_err() && Flag::parse("+ star white").is_err());
        assert!(Flag::parse("bar red wide green white").is_err() && Flag::parse("h red*0 white").is_err() && Flag::parse("star").is_err());
    }

    #[test]
    fn the_built_in_questions_are_sound() {
        let categories = builtin();
        assert!(categories.len() >= 4);
        let mut ids: Vec<&str> = categories.iter().map(|c| c.id.as_str()).collect();
        ids.dedup();
        assert_eq!(ids.len(), categories.len());
        for category in &categories {
            for level in Level::ALL {
                // Enough for two bridges with no question seen twice.
                assert!(category.count(level) >= 12, "{} has {} {} questions", category.id, category.count(level), level.name());
            }
            for (i, question) in category.questions.iter().enumerate() {
                // Everything a child reads can be drawn in the big letters.
                for text in question.wrong.iter().chain([&question.text, &question.answer]) {
                    assert!(crate::font::supported(text), "{}: no big letters for {text:?}", category.id);
                }
                assert!(question.text.ends_with('?'), "{}: {:?}", category.id, question.text);
                assert!(!question.fact.is_empty() && question.fact.ends_with(['.', '!']), "{}: the fact of {:?}", category.id, question.answer);
                let twice = category.questions[..i].iter().any(|q| q.text == question.text && q.answer == question.answer);
                assert!(!twice, "{}: {:?} is there twice", category.id, question.answer);
                // Two flags that look the same would make a question with two right answers.
                if let Some(flag) = &question.picture {
                    assert!(
                        category.questions[..i].iter().all(|q| q.picture.as_ref() != Some(flag)),
                        "{}: {:?} has another's flag",
                        category.id,
                        question.answer
                    );
                }
            }
        }
    }

    #[test]
    fn a_round_is_eight_right_answers_and_wrong_ones_come_back() {
        let category = Category::parse("sample", &format!("{SAMPLE}{MORE}")).unwrap();
        let mut rng = Rng::new(7);
        let mut round = Round::new(category.questions.clone(), &mut rng);
        assert_eq!(round.asked.question.answer, "Paris");
        assert_eq!(round.asked.options.len(), 4);
        assert_eq!(round.asked.options[round.asked.correct], "Paris");

        // A wrong answer lays nothing and the question is asked again later.
        assert!(!round.answer((round.asked.correct + 1) % 4));
        assert_eq!((round.planks, round.mistakes), (0, 1));
        let mut seen = Vec::new();
        while !round.built() {
            round.next(&mut rng);
            seen.push(round.asked.question.answer.clone());
            let mut options = round.asked.options.clone();
            options.sort();
            options.dedup();
            assert_eq!(options.len(), 4, "four different answers");
            assert!(round.answer(round.asked.correct));
        }
        // Five questions for eight planks: they go round again, never twice running.
        assert_eq!(&seen[..5], ["One", "Two", "Three", "Four", "Paris"]);
        assert!(seen.windows(2).all(|w| w[0] != w[1]));
        assert_eq!((round.planks, round.stars()), (PLANKS, 3));
        round.mistakes = 3;
        assert_eq!(round.stars(), 2);
        round.mistakes = 9;
        assert_eq!(round.stars(), 1);
    }

    #[test]
    fn a_players_own_files_join_or_replace_the_built_in_ones() {
        let dir = std::env::temp_dir().join(format!("fungeo-quiz-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (plain, problems) = load(&dir.join("missing"));
        assert!(problems.is_empty());
        let first = plain[0].id.clone();
        std::fs::write(dir.join("mine.txt"), "name: Mine\norder: -5\n[easy]\nQ: a?\nA: b\nX: c | d | e\n").unwrap();
        std::fs::write(dir.join(format!("{first}.txt")), "name: Replaced\n[easy]\nQ: a?\nA: b\nX: c | d | e\n").unwrap();
        std::fs::write(dir.join("broken.txt"), "[easy]\nQ: a?\n").unwrap();
        std::fs::write(dir.join("notes.md"), "not a category").unwrap();
        let (categories, problems) = load(&dir);
        assert_eq!(categories.len(), plain.len() + 1);
        assert_eq!(categories[0].name, "Mine");
        assert_eq!(categories.iter().find(|c| c.id == first).unwrap().name, "Replaced");
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("broken.txt") && problems[0].contains("line 2"), "{problems:?}");
        assert!(read(&dir.join("mix.txt")).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
