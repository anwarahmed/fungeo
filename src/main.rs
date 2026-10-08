mod app;
mod font;
mod quiz;
mod sound;
mod theme;
mod ui;
mod update;

use std::io::{self, IsTerminal, Write, stdout};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use ratatui::crossterm::execute;

use app::App;
use quiz::Level;
use theme::Settings;

const USAGE: &str = "\
fungeo - a geography quiz for children: build a bridge, one right answer at a time

Usage:
  fungeo                      play
  fungeo check [FILE...]      check question files of your own (all of them, if none
                              is named) and list every category
  fungeo update               check for a newer release now and install it
  fungeo update off | on      stop, or resume, checking when the game starts
  fungeo --help | --version

Options:
  -t, --theme NAME            sunny, ocean, candy, jungle or night
  -a, --animations on|off     whether things move (on by default)
  -s, --sound on|off          whether anything is heard (on by default)
      --no-intro              start at the passport, without the opening

The theme, the animations and the sound are remembered for next time, in
$XDG_STATE_HOME/fungeo (~/.local/state/fungeo). The stars are not: every start is a
fresh passport.

In the game everything can be clicked. With the keyboard:
  arrows, Enter               move the marker and choose
  A B C D  or  1 2 3 4        answer
  T theme   S sound   M motion (animations)   ? help
  Esc back to the passport    Q quit

Your own questions: put a text file for each category in
$XDG_CONFIG_HOME/fungeo/questions (~/.config/fungeo/questions). The README shows
the format, and `fungeo check` says whether a file is right.

Environment:
  FUNGEO_NO_UPDATE            set to skip the update check for one run
  FUNGEO_NO_SOUND             set to play no sound for one run
  FUNGEO_LOG                  a file to record every key and mouse event in
";

fn fail(msg: &str) -> ExitCode {
    eprintln!("fungeo: {msg}");
    ExitCode::FAILURE
}

/// `fungeo check`: reads the named files, or every file of the player's own, says
/// what is wrong with each, and lists the categories the game would show.
fn check(files: &[String]) -> ExitCode {
    let dir = quiz::own_dir();
    let mut bad = 0;
    if files.is_empty() {
        let (categories, problems) = quiz::load(&dir);
        println!("Your own question files are looked for in {}", dir.display());
        for category in &categories {
            let counts: Vec<String> = Level::ALL.iter().map(|&l| format!("{} {}", category.count(l), l.name().to_lowercase())).collect();
            println!("  {:<12} {:<14} {}", category.id, category.name, counts.join(", "));
        }
        for problem in &problems {
            eprintln!("fungeo: {problem}");
        }
        bad = problems.len();
    }
    for file in files {
        match quiz::read(&PathBuf::from(file)) {
            Ok(category) => {
                let counts: Vec<String> = Level::ALL.iter().map(|&l| format!("{} {}", category.count(l), l.name().to_lowercase())).collect();
                println!("{file}: \"{}\" is fine: {}", category.name, counts.join(", "));
                for level in Level::ALL.into_iter().filter(|&l| !category.has(l) && category.count(l) > 0) {
                    println!("{file}: note: {} needs at least {} questions to be played", level.name(), quiz::MIN_QUESTIONS);
                }
            }
            Err(e) => {
                eprintln!("fungeo: {file}: {e}");
                bad += 1;
            }
        }
    }
    if bad == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn main() -> ExitCode {
    let truecolor = matches!(std::env::var("COLORTERM").as_deref(), Ok("truecolor" | "24bit"));
    let settings_path = Settings::path();
    let mut settings = Settings::load(&settings_path);
    let mut intro = true;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "-V" | "-v" | "--version" => {
                println!("fungeo {} ({})", update::VERSION, if update::COMMIT.is_empty() { "unknown commit" } else { update::COMMIT });
                return ExitCode::SUCCESS;
            }
            "update" => {
                return match args.next().as_deref() {
                    None => update::command().map_or_else(|e| fail(&e), |()| ExitCode::SUCCESS),
                    Some(switch @ ("on" | "off")) => {
                        settings.update = switch == "on";
                        settings.save(&settings_path);
                        println!("The update check at startup is {switch}.");
                        ExitCode::SUCCESS
                    }
                    Some(_) => fail("'update' takes on, off or nothing"),
                };
            }
            "check" => return check(&args.collect::<Vec<_>>()),
            "-t" | "--theme" => match args.next().as_deref().and_then(theme::find_theme) {
                Some(theme) => settings.theme = theme,
                None => return fail(&format!("--theme needs one of: {}", theme::theme_names())),
            },
            "-a" | "--animations" => match args.next().as_deref() {
                Some(switch @ ("on" | "off")) => settings.animations = switch == "on",
                _ => return fail("--animations needs on or off"),
            },
            "-s" | "--sound" => match args.next().as_deref() {
                Some(switch @ ("on" | "off")) => settings.sound = switch == "on",
                _ => return fail("--sound needs on or off"),
            },
            "--no-intro" => intro = false,
            _ => return fail(&format!("unknown argument '{arg}'")),
        }
    }
    // What the options chose is remembered even when the game then cannot start.
    settings.save(&settings_path);
    if !io::stdin().is_terminal() || !stdout().is_terminal() {
        return fail("this is an interactive game and needs a terminal");
    }
    update::before_start(settings.update);

    let (categories, problems) = quiz::load(&quiz::own_dir());
    let mut app = App::new(truecolor, settings, categories, problems);
    app.settings_path = Some(settings_path);
    // Found even while sound is off, so that S has something to switch on.
    if std::env::var_os("FUNGEO_NO_SOUND").is_none() {
        app.speaker = sound::Speaker::find(update::state_dir().join("sounds"));
    }
    if intro {
        app.begin();
    }

    // Installed before ratatui's hook, which restores the terminal and then calls this one.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // ratatui's hook does not know the mouse was captured.
        let _ = execute!(stdout(), DisableMouseCapture);
        default_hook(info)
    }));

    match run(&mut app) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&e.to_string()),
    }
}

/// How long the loop waits for a key before drawing the water a little further on.
const POLL: Duration = Duration::from_millis(60);
/// The same while something is falling, walking or flying, so that it moves smoothly.
const FRAME: Duration = Duration::from_millis(16);

fn run(app: &mut App) -> io::Result<()> {
    // Raw mode, the alternate screen, and a panic hook that undoes both.
    let mut terminal = ratatui::init();
    // For clicking. While captured, selecting text needs shift (option on macOS).
    execute!(stdout(), EnableMouseCapture)?;
    // FUNGEO_LOG=file records every key and mouse event as the program receives it,
    // for working out why a terminal's input is not doing what it should.
    let mut log = std::env::var_os("FUNGEO_LOG").and_then(|path| std::fs::File::create(path).ok());
    let mut redraw = true;
    let result = loop {
        redraw |= app.tick();
        if redraw && let Err(e) = terminal.draw(|f| ui::draw(f, app)) {
            break Err(e);
        }
        redraw = false;
        if app.quit {
            break Ok(());
        }
        match event::poll(if app.animating() { FRAME } else { POLL }) {
            Ok(false) => {}
            Ok(true) => match event::read().inspect(|event| {
                if let Some(log) = &mut log {
                    let _ = writeln!(log, "{event:?}");
                }
            }) {
                Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => {
                    app.on_key(key);
                    redraw = true;
                }
                Ok(Event::Mouse(mouse)) => {
                    app.on_mouse(mouse);
                    redraw = true;
                }
                Ok(_) => redraw = true,
                Err(e) => break Err(e),
            },
            Err(e) => break Err(e),
        }
    };
    let _ = execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}
