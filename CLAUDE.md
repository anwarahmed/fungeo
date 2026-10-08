# fungeo

A geography quiz for children in the terminal (TUI): multiple-choice questions, each
right answer a plank of a bridge, stars shown in a "passport". Keyboard and mouse both
do everything. Rust + ratatui, targets macOS and Linux. `README.md` is for players and
parents; this file is for whoever changes the code.

## Commands

```sh
cargo run --release                          # play
cargo run --release -- --no-intro -t night   # straight to the passport, in a theme
cargo test                                   # unit tests: questions, rounds, app, drawing at seven window sizes, font, sound, settings, update
cargo clippy --all-targets -- -D warnings    # no warnings allowed
cargo fmt                                    # rustfmt.toml: max_width 160
cargo run -- check [FILE...]                 # validate question files, list categories
FUNGEO_LOG=/tmp/events.log cargo run         # record every key and mouse event received
```

`build.rs` stamps the commit for `--version` and writes the list of
`questions/*.txt` that `quiz.rs` includes, so a new built-in category is a new file
and nothing else.

## Status

Not published yet (October 2026). The plan is the same as funchess and wordl: the
`personal:publish-project` skill (GitHub repository, ruleset, CI, releases, install
script, Homebrew tap, AUR package). `src/update.rs` is already here, copied from
funchess with the names changed; it does nothing useful until there are releases, and
a build from a checkout never updates itself. When publishing, add the Workflow and
Releasing sections here the way funchess's `CLAUDE.md` has them, `install.sh`,
`tests/e2e.sh`, and the README's install section.

## Architecture

Single binary crate, no async, no threads. One file per concern in `src/`:

| File        | Role |
|-------------|------|
| `main.rs`   | CLI, terminal setup/teardown, the event loop (60 ms poll, 16 ms while something moves) |
| `quiz.rs`   | The question file format and its parser, `Category`, `Flag` (flags as shapes), `Round` (one bridge), the small `Rng`. Knows nothing about the screen |
| `app.rs`    | `App` state; what every key, click and tick does; `Progress` (the stars); particles |
| `ui.rs`     | All drawing: layouts, the river scene, flags, big-letter labels, and the clickable rectangles (`App::buttons`) |
| `font.rs`   | The 5x7 bitmap letters, drawn with half-block characters |
| `sound.rs`  | Sounds made out of notes, written as WAV files and handed to the system's player |
| `theme.rs`  | The five themes and `Settings` |
| `update.rs` | Self-update, the sister projects' design |

How a frame happens: `App::tick` advances time (`App::advance(dt)`, which tests call
directly), then `ui::draw` redraws everything and rebuilds `App::buttons`. A click
looks up the topmost button under it and calls `App::act(Action)`; a key is turned
into the same `Action`. So mouse and keyboard cannot drift apart: anything new must be
an `Action` with both a button and a key.

## Decisions, and why

- **Goal: journey + bridge.** The user chose it over a rocket, a map to fill in, and
  a bare bridge. A round is 8 planks (`quiz::PLANKS`); the passport (home screen)
  is the journey, a grid of category by level with the best stars of each.
- **Nobody loses.** A wrong answer costs a plank in the river and the question goes
  to the back of the queue to be met again. Stars count mistakes (0-1: three, 2-3:
  two, more: one). There are no lives, no timer and no score to fall.
- **Ages 5 to 13** (the user's choice). Easy is for early readers: famous places,
  short sentences. Hard is for 11 to 13. Every built-in question has a fact, which
  is where most of the learning is. The questions were written by Claude from general
  knowledge and not reviewed by a person: expect the user to report wrong or clumsy
  ones. Facts that go out of date (most people, tallest building, capital cities
  that move) are the ones to recheck.
- **Text size.** The user asked that text not be small. A terminal cannot scale
  text, so `ui::label` draws in `font.rs`'s big letters wherever they fit and falls
  back to bold text. `ui::play_layout` tries layouts from best to barely fitting
  (`Fit`); the question gets big letters before the answers do. The big font is
  capitals only; `font::supported` gates it, and a test requires every built-in
  question and answer to be drawable in it.
- **Categories are files** (the user asked for categories to be addable without
  touching the architecture). Built-in: `questions/*.txt`, listed by `build.rs`.
  A player's own: `~/.config/fungeo/questions/*.txt`, same format, read at start;
  a bad file is left out and counted on the passport, and `fungeo check` explains
  it. "Mix" is made in `App::new` out of every other category and has the id `mix`,
  which a file may not use. Stars are counted under the file's name.
- **Flags are shapes** (`quiz::Flag`): stripes, Nordic crosses, discs, a plus, a bar
  with stripes, and over any of them `quiz::Mark`s (a star, a disc, a triangle at
  the pole). Only flags that are recognizably those are in. A star is a few pixels
  in a small window, so `ui::play_layout` gives a flag the spare rows before the
  scene gets them. Two flags that draw
  the same (Indonesia/Monaco, Romania/Chad) would make a question with two right
  answers, so one of each pair is left out and a test compares every pair. Flag
  questions take their wrong answers from the other flags of the same level.
- **Keys:** plain letters, unlike funchess's Ctrl combinations, because nothing is
  typed here. `A`-`D` are answers, so animations are `M` (motion), not `A`. Ctrl
  with the same letters also works, since the handler ignores Ctrl except `Ctrl-C`.
- **The marker follows the mouse** (`MouseEventKind::Moved`), so there is one
  highlighted thing, not a keyboard one and a pointer one.
- **Sound** is funchess's design: no audio library, WAV files written to the state
  directory and played by `pw-play`/`paplay`/`aplay`/`afplay`. The plank's knock in
  `Sound::Right` is timed to `app::DROP`, and the notes of `Sound::Intro` to
  `app::INTRO_LETTER`; change them together.
- **Animations off** (`M`) means nothing waits: no intro, planks and the explorer
  jump to their places, the stars appear at once, and the screen is only redrawn on
  input.
- **Confetti is drawn behind everything** and sparkles in front, both only on empty
  cells, so neither covers a word.
- **Colors** are 24-bit; without `COLORTERM=truecolor` the finished frame is mapped
  to the 256-color cube (`ui::indexed`).
- **A deck per category and level** (`App::decks`) lasts for the session, so a second
  round brings questions the first did not. Each start shuffles afresh
  (`Rng::from_clock`). The user asked for more variety after the first version,
  which had about 16 questions a level and dealt 12 of them to every round; there
  are now about 40 a level (flags about 20). Keep a level well above the 12 a round
  is dealt, or every round looks the same.
- **Stars are not kept.** The user asked (2026-10-08) for the stars to reset every
  time the game starts, so `Progress` lives in memory only and nothing is written.
  An older `progress` file in the state directory, from before, is ignored.

## Verifying a change

- `cargo test` draws every screen at seven sizes from 60x22 to 300x90 in all themes
  and plays whole rounds; a layout that puts a button off the window fails it.
- To see it: run it in tmux on a **private socket** (`tmux -L fungeo-test ...`; never
  the user's server), `capture-pane -p -e -N`, and turn that into a picture. The
  script that does this for funchess is `docs/screenshot.sh` in that repository;
  note its converter resets colors at each line and only knows `▀`, which is wrong
  for this game (`▄`, `█`, and tmux carrying colors across lines).
- A scripted player cannot know the right answer; guessing `a` then `Enter` until
  "Cross!" appears finishes a round in about 30 tries. Check for "Cross!" between
  the two keys, or the `Enter` walks past the end.

## Known gaps

- The sounds were never heard by whoever made them: checked as numbers only. The
  intro tune's timing against the falling letters is by arithmetic.
- The animations were only seen as captured frames, not live.
- Never run on a Mac.
- The big font has no lower case, and a question in a non-Latin script falls back to
  ordinary text.
- No flags with emblems, crescents, cantons or diagonals, so many well-known flags
  (USA, UK, Canada, Brazil, China, Turkey) are missing.
- Ideas offered and not built: a map that fills in, a rocket, a daily challenge,
  profiles, a "learn" mode that shows the facts without questions, pictures for
  landmarks.
