# fungeo

A geography quiz for children, in the terminal, on Linux and macOS.

Every right answer lays a plank of a bridge. Eight planks and the explorer walks
across the river, the confetti falls, and the round earns up to three stars in the
passport.

- **Four kinds of question, and a mix of them all:** countries and capitals, flags
  (65 of them, drawn on the screen), nature (oceans, rivers, mountains, animals) and wonders
  (famous places, languages, food).
- **Three levels:** easy for early readers, medium for about 8 to 10, hard for 11
  to 13. More than 450 questions in all, each with something worth knowing shown
  after the answer. They come in a different order every time, and a second round of
  the same square brings questions the first did not.
- **Mouse or keyboard, or both at once.** Everything on the screen can be clicked,
  and everything has a key.
- **Big letters.** In a large window the questions and answers are drawn several rows
  tall. A terminal cannot change its own text size, so for the smaller text make the
  window full screen and zoom in (usually `Ctrl` `+`).
- **Kind to a wrong answer.** A plank falls in the river, the right answer is shown,
  and the question comes back later for another try. Nobody loses.
- **Five color themes**, animations, sounds, and an opening with a tune.

## Play

```sh
fungeo
```

| | Mouse | Keys |
|---|---|---|
| Choose a category and level | click its square | arrows, then `Enter` |
| Answer | click it | `A` `B` `C` `D` or `1` `2` `3` `4`, or arrows and `Enter` |
| Next question | click **Next** | `Enter` or `Space` |
| Back to the passport | click **Back** | `Esc` |
| Theme, sound, motion | click them | `T`, `S`, `M` |
| Help, quit | click them | `?`, `Q` |

Three stars for a bridge built with one mistake or none, two for up to three
mistakes, one for getting across at all. The passport shows the best of each
category and level since the game was started; every start is a fresh passport.

Options: `--theme sunny|ocean|candy|jungle|night`, `--sound on|off`,
`--animations on|off`, `--no-intro`. `fungeo --help` lists everything. The window
needs to be at least 60 columns by 22 rows; big letters appear from about 120 by 36.

Sound needs nothing installed: it is played with `pw-play`, `paplay` or `aplay` on
Linux and `afplay` on macOS, whichever is there.

## Questions of your own

A category is one text file. Put it in `~/.config/fungeo/questions/` (or
`$XDG_CONFIG_HOME/fungeo/questions/`) and it appears in the passport:

```text
name: Our Town
color: #00897b
order: 10

[easy]
Q: Which river runs through our town?
A: The Avon
X: The Nile | The Thames | The Amazon
F: It rises in the hills to the north.

Q: Which country do we live in?
A: England
X: France | Spain | Italy

[hard]
...
```

- `Q` is the question, `A` the right answer, `X` three or more wrong ones (three are
  shown each time), `F` an optional fact for afterwards. Blank lines separate
  questions.
- `[easy]`, `[medium]` and `[hard]` start a level. A level can be played once it
  has four questions; eight or more make a better round.
- `name`, `color` (`#rrggbb`) and `order` (lower is higher up; the built-in ones are
  1 to 4) are optional.
- A flag: `P: flag h red white blue` (stripes top to bottom), `v` (side by side),
  `nordic FIELD CROSS`, `disc FIELD DISC`, `plus FIELD CROSS`, or
  `bar COLOR WIDTH STRIPES...`. `red*2` makes a stripe twice as thick. A star, a
  circle or a triangle at the pole goes on top after a `+`:
  `P: flag h red red + star yellow 30`.
- With an `ask: ...` line at the top, questions may leave out `Q`; and questions
  without `X` use one another's answers as the wrong ones. That is how the flags
  file is written: see [questions/flags.txt](questions/flags.txt).
- A file named like a built-in one (`nature.txt`) replaces it.

`fungeo check` lists every category and says what is wrong with a file, by line.

## From source

Needs Rust 1.88 or newer.

```sh
cargo build --release
target/release/fungeo
```

## Files

| What | Where |
|---|---|
| Settings, generated sound files | `~/.local/state/fungeo/` (`$XDG_STATE_HOME`) |
| Your own questions | `~/.config/fungeo/questions/` (`$XDG_CONFIG_HOME`) |

## License

MIT. See [LICENSE](LICENSE).
