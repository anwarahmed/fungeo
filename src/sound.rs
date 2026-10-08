//! Sound effects. Every sound is made here out of a few notes, written once as a WAV
//! file, and played by handing that file to a program the system already has
//! (`pw-play`, `paplay` or `aplay` on Linux, `afplay` on macOS). Nothing is linked
//! against an audio library, and where no such program exists the game is silent.

use std::f32::consts::TAU;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// Samples a second, of one channel of 16 bits.
const RATE: u32 = 22_050;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sound {
    /// The tune the game opens with: one note for each letter of its name.
    Intro,
    /// A round begins.
    Start,
    /// A button that is neither right nor wrong.
    Click,
    /// A right answer, and its plank landing on the bridge.
    Right,
    /// A wrong answer, and its plank falling in the river.
    Wrong,
    /// A footstep on the bridge.
    Step,
    /// The bridge is crossed.
    Win,
    /// One star of the stars a round earns.
    Star,
    /// The tune of a celebration, from the smallest (1) to the grandest (6); see
    /// `crate::app::Feat::tier`.
    Fanfare(u8),
}

#[derive(Clone, Copy)]
enum Wave {
    /// A pure tone: a bell.
    Sine,
    /// A brighter one.
    Triangle,
    /// A knock: a tone that drops as it dies away, which is what wood does.
    Knock,
    /// A hiss.
    Noise,
}

/// One note: what it sounds like, its pitch in Hz, when it starts and how long it
/// lasts in seconds, and how loud it is, from 0 to 1.
type Note = (Wave, f32, f32, f32, f32);

// The notes of the tunes, in Hz.
const C4: f32 = 261.6;
const E4: f32 = 329.6;
const C5: f32 = 523.3;
const D5: f32 = 587.3;
const E5: f32 = 659.3;
const F5: f32 = 698.5;
const G5: f32 = 784.0;
const A5: f32 = 880.0;
const B5: f32 = 987.8;
const C6: f32 = 1046.5;
const D6: f32 = 1174.7;
const E6: f32 = 1318.5;
const G6: f32 = 1568.0;
const C7: f32 = 2093.0;

/// A tune being written: its notes so far, and the moment the next one starts.
struct Tune {
    notes: Vec<Note>,
    at: f32,
}

impl Tune {
    /// Notes one after another, `every` seconds apart, each with a tap of the drum.
    fn run(&mut self, pitches: &[f32], every: f32) {
        for &pitch in pitches {
            self.notes.push((Wave::Triangle, pitch, self.at, every * 1.6, 0.28));
            self.notes.push((Wave::Knock, 200.0, self.at, 0.08, 0.25));
            self.at += every;
        }
    }

    /// Notes together, on a beat of the big drum. The next thing comes `then`
    /// seconds later.
    fn chord(&mut self, pitches: &[f32], lasts: f32, then: f32) {
        for &pitch in pitches {
            self.notes.push((Wave::Triangle, pitch, self.at, lasts, 0.15));
        }
        self.notes.push((Wave::Knock, 130.0, self.at, 0.15, 0.25));
        self.at += then;
    }

    /// A roll of the drum that grows louder.
    fn roll(&mut self, taps: usize) {
        for i in 0..taps {
            self.notes.push((Wave::Noise, 0.0, self.at, 0.05, 0.08 + 0.012 * i as f32));
            self.notes.push((Wave::Knock, 160.0, self.at, 0.05, 0.15));
            self.at += 0.05;
        }
    }

    /// Bells ringing up over the chord that was just struck.
    fn bells(&mut self, pitches: &[f32]) {
        for (i, &pitch) in pitches.iter().enumerate() {
            self.notes.push((Wave::Sine, pitch, self.at + 0.12 + i as f32 * 0.12, 0.5, 0.16));
        }
    }
}

/// The tune of a celebration. Each is longer than the one before it and has more in
/// it: a drum roll from the fourth on, a second verse in the fifth, and the sixth
/// opens with the notes the game itself opens with.
fn fanfare(tier: u8) -> Vec<Note> {
    const C: [f32; 4] = [C5, E5, G5, C6];
    const F: [f32; 3] = [F5, A5, C6];
    const G: [f32; 3] = [G5, B5, D6];
    let mut tune = Tune { notes: Vec::new(), at: 0.0 };
    match tier {
        // Ta-ta-ta-daa.
        0 | 1 => {
            tune.run(&[G5, G5, G5], 0.13);
            tune.chord(&C, 0.9, 0.0);
        }
        2 => {
            tune.run(&[C5, E5, G5], 0.12);
            tune.chord(&C[..3], 0.3, 0.24);
            tune.run(&[G5], 0.12);
            tune.chord(&C, 1.0, 0.0);
        }
        3 | 4 => {
            if tier == 4 {
                tune.roll(6);
            }
            let (up, rung): (&[f32], &[f32]) = if tier == 4 { (&[C5, E5, G5, C6, E6], &[E6, G6, C7]) } else { (&C, &[E6, G6]) };
            tune.run(up, 0.12);
            tune.chord(&F, 0.4, 0.3);
            tune.chord(&G, 0.4, 0.3);
            tune.chord(&C, if tier == 4 { 1.4 } else { 1.1 }, 0.0);
            tune.bells(rung);
        }
        _ => {
            if tier >= 6 {
                tune.run(&[C5, D5, E5, G5, A5, C6], 0.14);
            }
            tune.roll(8);
            tune.run(&C, 0.12);
            tune.chord(&F, 0.4, 0.3);
            tune.chord(&G, 0.4, 0.3);
            tune.run(&[G5, B5, D6, G6], 0.12);
            tune.chord(&C, 0.4, 0.3);
            tune.chord(&F, 0.4, 0.3);
            tune.chord(&G, 0.4, 0.3);
            if tier >= 6 {
                tune.roll(10);
                tune.chord(&C, 0.35, 0.3);
                tune.chord(&C, 0.35, 0.3);
                tune.chord(&C, 2.0, 0.0);
                tune.bells(&[E6, G6, C7, E6, G6, C7, C7]);
            } else {
                tune.chord(&C, 1.6, 0.0);
                tune.bells(&[E6, G6, C7, G6, C7]);
            }
        }
    }
    tune.notes
}

impl Sound {
    #[cfg(test)]
    pub const ALL: [Sound; 14] = [
        Sound::Intro,
        Sound::Start,
        Sound::Click,
        Sound::Right,
        Sound::Wrong,
        Sound::Step,
        Sound::Win,
        Sound::Star,
        Sound::Fanfare(1),
        Sound::Fanfare(2),
        Sound::Fanfare(3),
        Sound::Fanfare(4),
        Sound::Fanfare(5),
        Sound::Fanfare(6),
    ];

    fn name(self) -> &'static str {
        match self {
            Sound::Intro => "intro",
            Sound::Start => "start",
            Sound::Click => "click",
            Sound::Right => "right",
            Sound::Wrong => "wrong",
            Sound::Step => "step",
            Sound::Win => "win",
            Sound::Star => "star",
            Sound::Fanfare(tier) => ["fanfare1", "fanfare2", "fanfare3", "fanfare4", "fanfare5", "fanfare6"][tier.clamp(1, 6) as usize - 1],
        }
    }

    fn notes(self) -> Vec<Note> {
        use Wave::{Knock, Noise, Sine, Triangle};
        let notes: &[Note] = match self {
            Sound::Fanfare(tier) => return fanfare(tier),
            // Six letters drop in, `crate::app::INTRO_LETTER` seconds apart, each with
            // a knock as it lands, and then a chord.
            Sound::Intro => &[
                (Triangle, C5, 0.25, 0.3, 0.3),
                (Knock, 200.0, 0.25, 0.1, 0.4),
                (Triangle, D5, 0.5, 0.3, 0.3),
                (Knock, 200.0, 0.5, 0.1, 0.4),
                (Triangle, E5, 0.75, 0.3, 0.3),
                (Knock, 200.0, 0.75, 0.1, 0.4),
                (Triangle, G5, 1.0, 0.3, 0.3),
                (Knock, 200.0, 1.0, 0.1, 0.4),
                (Triangle, A5, 1.25, 0.3, 0.3),
                (Knock, 200.0, 1.25, 0.1, 0.4),
                (Triangle, C6, 1.5, 0.35, 0.3),
                (Knock, 200.0, 1.5, 0.1, 0.4),
                (Triangle, C5, 1.95, 0.9, 0.2),
                (Triangle, E5, 1.95, 0.9, 0.2),
                (Triangle, G5, 1.95, 0.9, 0.2),
                (Sine, C6, 1.95, 0.9, 0.25),
                (Sine, E6, 2.1, 0.75, 0.12),
            ],
            Sound::Start => &[(Triangle, C5, 0.0, 0.12, 0.3), (Triangle, G5, 0.1, 0.22, 0.3)],
            Sound::Click => &[(Triangle, G5, 0.0, 0.07, 0.25)],
            // Two bells going up, then wood on wood as the plank lands
            // (`crate::app::DROP` seconds after the answer).
            Sound::Right => &[(Sine, E5, 0.0, 0.14, 0.3), (Sine, C6, 0.1, 0.3, 0.3), (Knock, 260.0, 0.45, 0.12, 0.6), (Noise, 0.0, 0.45, 0.02, 0.2)],
            // Down instead of up, and softly: it is no fun to be laughed at. Then a plop.
            Sound::Wrong => &[(Sine, E4, 0.0, 0.2, 0.25), (Sine, C4, 0.16, 0.3, 0.25), (Knock, 150.0, 0.55, 0.18, 0.45), (Noise, 0.0, 0.55, 0.12, 0.15)],
            Sound::Step => &[(Knock, 380.0, 0.0, 0.07, 0.5)],
            Sound::Win => &[
                (Triangle, C5, 0.0, 0.14, 0.3),
                (Triangle, E5, 0.12, 0.14, 0.3),
                (Triangle, G5, 0.24, 0.14, 0.3),
                (Triangle, C6, 0.36, 0.6, 0.3),
                (Sine, E6, 0.36, 0.6, 0.15),
                (Sine, G6, 0.36, 0.6, 0.12),
            ],
            Sound::Star => &[(Sine, C6, 0.0, 0.25, 0.3), (Sine, G6, 0.03, 0.22, 0.15)],
        };
        notes.to_vec()
    }

    /// The sound itself.
    fn samples(self) -> Vec<i16> {
        let notes = self.notes();
        let length = notes.iter().map(|&(_, _, start, lasts, _)| start + lasts).fold(0.0, f32::max);
        let mut mix = vec![0.0f32; (length * RATE as f32) as usize];
        let mut seed = 0x2545_F491u32;
        for &(wave, pitch, start, lasts, loud) in &notes {
            let first = (start * RATE as f32) as usize;
            for i in 0..(lasts * RATE as f32) as usize {
                let t = i as f32 / RATE as f32;
                let turn = (t * pitch).fract();
                let value = match wave {
                    Wave::Sine => (turn * TAU).sin(),
                    Wave::Triangle => 4.0 * (turn - 0.5).abs() - 1.0,
                    Wave::Knock => (TAU * pitch * (t - 2.0 * t * t)).sin(),
                    Wave::Noise => {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        seed as f32 / u32::MAX as f32 * 2.0 - 1.0
                    }
                };
                // In quickly, so that it does not click, and then dying away.
                let shape = (t / 0.004).min(1.0) * (-5.0 * t / lasts).exp() * (1.0 - t / lasts);
                if let Some(sample) = mix.get_mut(first + i) {
                    *sample += value * shape * loud;
                }
            }
        }
        mix.into_iter().map(|v| (v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).collect()
    }

    /// The sound as a WAV file.
    fn wav(self) -> Vec<u8> {
        let samples = self.samples();
        let bytes = samples.len() as u32 * 2;
        let mut out = Vec::with_capacity(44 + bytes as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + bytes).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        // 16 bytes of format: plain samples, one channel, the rate, bytes a second,
        // bytes a sample and bits a sample.
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&RATE.to_le_bytes());
        out.extend_from_slice(&(RATE * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&bytes.to_le_bytes());
        out.extend(samples.into_iter().flat_map(i16::to_le_bytes));
        out
    }
}

/// What plays the sounds, or does not.
pub struct Speaker {
    /// The program that plays a WAV file given as its last argument, and where the
    /// files are kept. Nothing when the game is silent.
    player: Option<(PathBuf, &'static [&'static str], PathBuf)>,
    written: Vec<Sound>,
    /// The players still playing, so that none is left behind as a zombie.
    playing: Vec<Child>,
    /// Everything asked for, in order.
    #[cfg(test)]
    pub heard: Vec<Sound>,
}

impl Speaker {
    pub fn silent() -> Speaker {
        Speaker {
            player: None,
            written: Vec::new(),
            playing: Vec::new(),
            #[cfg(test)]
            heard: Vec::new(),
        }
    }

    /// Plays through `program`, keeping the sound files in `dir`.
    pub fn through(program: PathBuf, arguments: &'static [&'static str], dir: PathBuf) -> Speaker {
        Speaker { player: Some((program, arguments, dir)), ..Speaker::silent() }
    }

    /// Plays through whichever of the system's own players is installed, looked for
    /// along `PATH`. Silent when there is none.
    pub fn find(dir: PathBuf) -> Speaker {
        let players: &[(&str, &[&str])] = if cfg!(target_os = "macos") { &[("afplay", &[])] } else { &[("pw-play", &[]), ("paplay", &[]), ("aplay", &["-q"])] };
        let path = std::env::var_os("PATH").unwrap_or_default();
        for &(name, arguments) in players {
            if let Some(program) = std::env::split_paths(&path).map(|dir| dir.join(name)).find(|file| file.is_file()) {
                return Speaker::through(program, arguments, dir);
            }
        }
        Speaker::silent()
    }

    /// Whether there is anything to play a sound with.
    #[cfg(test)]
    pub fn is_on(&self) -> bool {
        self.player.is_some()
    }

    /// Starts the sound and returns at once. A sound that cannot be played is not
    /// worth interrupting a game for.
    pub fn play(&mut self, sound: Sound) {
        #[cfg(test)]
        self.heard.push(sound);
        let Some((program, arguments, dir)) = &self.player else { return };
        self.playing.retain_mut(|child| !matches!(child.try_wait(), Ok(Some(_)) | Err(_)));
        let file = dir.join(format!("{}.wav", sound.name()));
        if !self.written.contains(&sound) {
            if std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&file, sound.wav())).is_err() {
                return;
            }
            self.written.push(sound);
        }
        // It must not write on the screen the game is drawn on.
        let child = Command::new(program).args(*arguments).arg(&file).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
        self.playing.extend(child);
    }

    /// Where a sound's file is kept, once it has been played.
    #[cfg(test)]
    fn file(&self, sound: Sound) -> Option<PathBuf> {
        self.player.as_ref().map(|(_, _, dir)| dir.join(format!("{}.wav", sound.name())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sound_is_short_heard_and_never_too_loud() {
        for sound in Sound::ALL {
            let samples = sound.samples();
            let seconds = samples.len() as f32 / RATE as f32;
            // Only a celebration may go on for longer than three seconds.
            let longest = if matches!(sound, Sound::Fanfare(_)) { 8.0 } else { 3.0 };
            assert!((0.05..longest).contains(&seconds), "{sound:?} lasts {seconds}");
            let loudest = samples.iter().map(|s| s.unsigned_abs()).max().unwrap();
            assert!((3000..30000).contains(&loudest), "{sound:?} peaks at {loudest}");
            // It starts and ends in silence, so nothing clicks.
            assert!(samples[0].abs() < 600 && samples[samples.len() - 1].abs() < 600, "{sound:?}");

            let wav = sound.wav();
            assert_eq!((&wav[..4], &wav[8..16], &wav[36..40]), (&b"RIFF"[..], &b"WAVEfmt "[..], &b"data"[..]));
            assert_eq!(u32::from_le_bytes(wav[4..8].try_into().unwrap()) as usize, wav.len() - 8);
            assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize, samples.len() * 2);
        }
        let names: std::collections::HashSet<_> = Sound::ALL.into_iter().map(Sound::name).collect();
        assert_eq!(names.len(), Sound::ALL.len());
    }

    #[test]
    fn a_grander_celebration_has_a_longer_tune() {
        let lengths: Vec<usize> = (1..=6).map(|tier| Sound::Fanfare(tier).samples().len()).collect();
        assert!(lengths.windows(2).all(|pair| pair[0] < pair[1]), "{lengths:?}");
        // The smallest is over quickly, and the grandest is a tune of its own.
        assert!(lengths[0] < 2 * RATE as usize && lengths[5] > 5 * RATE as usize, "{lengths:?}");
    }

    /// A stand-in for the system's player, which notes what it was asked to play.
    #[test]
    fn a_sound_is_a_file_handed_to_the_systems_player() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("fungeo-sound-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (player, log) = (dir.join("player"), dir.join("log"));
        std::fs::write(&player, format!("#!/bin/sh\necho \"$@\" >> '{}'\n", log.display())).unwrap();
        std::fs::set_permissions(&player, std::fs::Permissions::from_mode(0o755)).unwrap();

        let mut speaker = Speaker::through(player, &["-q"], dir.join("sounds"));
        assert!(speaker.is_on());
        speaker.play(Sound::Click);
        speaker.play(Sound::Win);
        speaker.play(Sound::Click);
        for child in &mut speaker.playing {
            child.wait().unwrap();
        }
        let (moved, won) = (speaker.file(Sound::Click).unwrap(), speaker.file(Sound::Win).unwrap());
        let mut asked: Vec<String> = std::fs::read_to_string(&log).unwrap().lines().map(String::from).collect();
        asked.sort();
        assert_eq!(asked, [format!("-q {}", moved.display()), format!("-q {}", moved.display()), format!("-q {}", won.display())]);
        assert_eq!(std::fs::read(&moved).unwrap(), Sound::Click.wav());
        assert_eq!(speaker.heard, [Sound::Click, Sound::Win, Sound::Click]);

        // With no player there is nothing to hear and nothing is written.
        let mut speaker = Speaker::silent();
        speaker.play(Sound::Step);
        assert!(!speaker.is_on() && speaker.playing.is_empty() && speaker.written.is_empty());
        // A player that is not there is no reason to stop.
        let mut speaker = Speaker::through(dir.join("missing"), &[], dir.join("sounds"));
        speaker.play(Sound::Step);
        assert!(speaker.playing.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
