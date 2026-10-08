# fungeo

A geography quiz for children in the terminal (TUI): multiple-choice questions, each
right answer a plank of a bridge, stars shown in a "passport". Keyboard and mouse both
do everything. Rust + ratatui, targets macOS and Linux. `README.md` is for players and
parents; this file is for whoever changes the code.

## Commands

```sh
cargo run --release                          # play (from a checkout it never updates itself)
cargo run --release -- --no-intro -t night   # straight to the passport, in a theme
cargo test                                   # unit tests: questions, rounds, celebrations, app, drawing at seven window sizes, font, sound, settings, update
cargo clippy --all-targets -- -D warnings    # no warnings allowed
cargo fmt                                    # rustfmt.toml: max_width 160
cargo run -- check [FILE...]                 # validate question files, list categories
cargo build --release && tests/e2e.sh        # the built program in tmux, install.sh, the updater
FUNGEO_LOG=/tmp/events.log cargo run         # record every key and mouse event received
```

`build.rs` stamps the commit for `--version` and writes the list of
`questions/*.txt` that `quiz.rs` includes, so a new built-in category is a new file
and nothing else.

`install.sh` (POSIX sh, macOS + Linux) downloads the latest release binary into
`~/.local/bin` (`FUNGEO_BIN_DIR` overrides) and verifies its checksum. `--source`
builds instead (the checkout it is in, else a fresh clone), and it falls back to that
when no binary exists for the platform. `--link` symlinks to the checkout's build,
`--uninstall` removes it. It never installs Rust and never edits shell profiles.
`FUNGEO_RELEASE_URL` points it, and the self-updater, at another download base (a
`file://` directory holding `VERSION`, `SHA256SUMS` and a binary; `tests/e2e.sh` makes
such directories).

## Workflow

- **`main` only accepts pull requests** (GitHub ruleset "Main"): no direct pushes, no
  force-pushes, no deletion, no bypass for anyone. A PR needs these checks to pass,
  matched by job name: `test (ubuntu-latest)`, `test (macos-latest)`, `msrv`,
  `release checklist`. Renaming a CI job means updating the ruleset or PRs wait forever.
  PRs are squash-merged.
- **Local layout.** The user keeps this repo as a bare clone with one worktree per
  branch: `~/Developer/GitHub/anwarahmed/fungeo/main` plus a sibling directory per
  feature branch (`git worktree add -b <branch> <branch> origin/main` from the bare
  repo). Remove the worktree and branch after the PR merges, then fast-forward `main`.
- **Releasing** is merging a version bump; a merge without one publishes nothing.
  **Follow [RELEASING.md](RELEASING.md) every time, every step.** The release PR's
  description must carry its checklist with every line ticked (`gh pr create --body`
  does not add it for you), or the `release checklist` check fails. Tick a line only
  after doing what it says. Then do its "After merging" steps and report each one.
- **Sibling repo:** https://github.com/anwarahmed/homebrew-tap holds the generated
  Homebrew formula (`Formula/fungeo.rb`, written by its
  `scripts/formulae/fungeo.sh`). It takes direct pushes, because its bot commits
  formulae to `main`.
- **Sister projects:** funchess, wordl and typeshelf (same owner) share this release workflow,
  `install.sh` and `src/update.rs` design. A fix to any of those in one project belongs
  in the others in the same sitting.

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
  `H` `J` `K` `L` are the arrows (vim's), turned into them at the top of
  `App::on_key`, so they do whatever the arrows do on every screen. A new letter key
  must avoid those four as well as `A`-`D`, `T`, `S`, `M`, `N`, `P` and `Q`.
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
- **A click counts on press; a release with no press before it counts too**, in case
  a terminal only reports the release. This came from funchess, where the user once
  reported clicks not registering in foot under tmux. If it happens here, ask for a
  `FUNGEO_LOG` recording before guessing.
- **Self-update** is the sister projects' design exactly: a `VERSION` file from the
  latest release (no GitHub API, so no rate limit), a `share/fungeo/managed-by` marker
  by which Homebrew and pacman switch it off, and a check at most once a day
  (`last-update-check` in the state directory). The release asset names
  (`fungeo-<target>`, `SHA256SUMS`, `VERSION`, `PKGBUILD`) are a contract with
  `install.sh`, the updater, the tap's formula generator and the AUR package.
- **Colors** are 24-bit; without `COLORTERM=truecolor` the finished frame is mapped
  to the 256-color cube (`ui::indexed`).
- **A deck per category and level** (`App::decks`) lasts for the session, so a second
  round brings questions the first did not. Each start shuffles afresh
  (`Rng::from_clock`). The user asked for more variety after the first version,
  which had about 16 questions a level and dealt 12 of them to every round; there
  are now about 40 a level (flags about 20). Keep a level well above the 12 a round
  is dealt, or every round looks the same.
- **Celebrations** (the user's list, 2026-10-08, since 0.1.2, from smallest to grandest): a
  category finished, Easy in every category, Medium, Hard, every square of the
  passport, and every square with three stars. They are `app::Feat`, and
  `Feat::tier` (1 to 6) is what everything about one grows with: the tune
  (`Sound::Fanfare`), the confetti and fireworks (`App::celebrate`, `App::advance`)
  and the window (`ui::celebration`: a medal in the category's color, bronze, silver
  or gold, then a cup, then the cup between two medals with the title in the colors
  of the game's name). `App::finish` compares the feats before and after the round's
  stars and celebrates only the grandest new one, after the stars have appeared;
  going on before that brings it at once, so it cannot be missed, and for its first
  `app::SEEN` seconds no key closes it. "Mix" counts as a category. Since stars are
  not kept, each can be earned once per start. Its fireworks and front confetti are
  drawn over the window but under its words.
- **Stars are not kept.** The user asked (2026-10-08) for the stars to reset every
  time the game starts, so `Progress` lives in memory only and nothing is written.
  An older `progress` file in the state directory, from before, is ignored.

## Verifying a change

- `cargo test` draws every screen at seven sizes from 60x22 to 300x90 in all themes
  and plays whole rounds; a layout that puts a button off the window fails it.
- `cargo build --release && tests/e2e.sh`: the real program in tmux on a private
  socket, by keyboard and by mouse, a whole round, the sounds (through stand-in
  players), and `install.sh` and the updater against made-up releases served from
  `file://`. CI runs it on Linux and macOS.
- To see it: `docs/screenshot.sh` runs it in tmux on a **private socket** (never the
  user's server), captures the pane with its colors (`capture-pane -p -e -N`) and
  draws that as a picture. Pass it a size and keys to look at any screen.
- A scripted player cannot know the right answer; guessing `a` then `Enter` until
  "Cross!" appears finishes a round in about 30 tries. Check for "Cross!" between
  the two keys, or the `Enter` walks past the end.

## Known gaps

- The sounds were never heard by whoever made them: checked as numbers only. The
  intro tune's timing against the falling letters is by arithmetic. The exception
  is the six celebration tunes, which the user listened to on Linux (2026-10-08,
  through `pw-play`) and liked; they have not been played through `afplay`.
- `tests/e2e.sh` does not reach a celebration: it would take three rounds of
  guessing. The unit tests play the whole passport instead.
- The animations were only seen as captured frames, not live.
- Never run by a person on a Mac; CI runs the tests there.
- The AUR package `fungeo-bin` is rendered for each release but not pushed: the user
  has no AUR account.
- The big font has no lower case, and a question in a non-Latin script falls back to
  ordinary text.
- No flags with emblems, crescents, cantons or diagonals, so many well-known flags
  (USA, UK, Canada, Brazil, China, Turkey) are missing.
- Ideas offered and not built: a map that fills in, a rocket, a daily challenge,
  profiles, a "learn" mode that shows the facts without questions, pictures for
  landmarks.
