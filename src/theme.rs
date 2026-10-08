//! Color themes, and the settings that are remembered between runs.

use std::path::PathBuf;

/// Red, green and blue.
pub type Rgb = (u8, u8, u8);

pub struct Theme {
    pub name: &'static str,
    /// The window, and the panels drawn on it.
    pub bg: Rgb,
    pub panel: Rgb,
    pub text: Rgb,
    pub dim: Rgb,
    /// The title's second color, the marker, the explorer's hat.
    pub accent: Rgb,
    /// The four answers: A, B, C and D.
    pub answers: [Rgb; 4],
    pub good: Rgb,
    pub bad: Rgb,
    /// The sky at the top and at the horizon.
    pub sky: [Rgb; 2],
    pub water: [Rgb; 2],
    pub grass: Rgb,
    pub wood: Rgb,
    /// The sun, or the moon when `night`.
    pub sun: Rgb,
    /// Stars in the sky instead of clouds.
    pub night: bool,
}

pub const THEMES: [Theme; 5] = [
    Theme {
        name: "sunny",
        bg: (255, 248, 225),
        panel: (255, 236, 179),
        text: (62, 39, 35),
        dim: (141, 110, 99),
        accent: (255, 112, 67),
        answers: [(239, 83, 80), (66, 165, 245), (255, 202, 40), (102, 187, 106)],
        good: (46, 160, 67),
        bad: (211, 47, 47),
        sky: [(79, 179, 240), (214, 240, 255)],
        water: [(30, 136, 229), (100, 181, 246)],
        grass: (124, 179, 66),
        wood: (161, 110, 60),
        sun: (255, 214, 0),
        night: false,
    },
    Theme {
        name: "ocean",
        bg: (10, 37, 64),
        panel: (18, 58, 99),
        text: (232, 244, 253),
        dim: (144, 180, 210),
        accent: (0, 229, 255),
        answers: [(255, 107, 107), (78, 205, 196), (255, 217, 61), (165, 137, 255)],
        good: (67, 200, 110),
        bad: (255, 99, 99),
        sky: [(22, 96, 160), (130, 205, 235)],
        water: [(0, 105, 170), (0, 160, 210)],
        grass: (82, 183, 136),
        wood: (200, 155, 105),
        sun: (255, 244, 170),
        night: false,
    },
    Theme {
        name: "candy",
        bg: (255, 240, 246),
        panel: (255, 214, 231),
        text: (90, 24, 74),
        dim: (173, 104, 148),
        accent: (255, 64, 129),
        answers: [(255, 105, 180), (149, 117, 255), (38, 198, 218), (255, 171, 64)],
        good: (38, 166, 91),
        bad: (233, 30, 99),
        sky: [(255, 170, 205), (255, 236, 210)],
        water: [(126, 142, 255), (186, 170, 255)],
        grass: (120, 220, 170),
        wood: (214, 120, 150),
        sun: (255, 250, 160),
        night: false,
    },
    Theme {
        name: "jungle",
        bg: (20, 45, 30),
        panel: (32, 70, 45),
        text: (236, 247, 225),
        dim: (150, 185, 150),
        accent: (255, 193, 7),
        answers: [(244, 110, 60), (41, 182, 246), (253, 216, 53), (156, 204, 101)],
        good: (102, 210, 110),
        bad: (255, 110, 90),
        sky: [(90, 190, 150), (225, 245, 200)],
        water: [(0, 121, 107), (60, 180, 165)],
        grass: (56, 142, 60),
        wood: (141, 100, 80),
        sun: (255, 241, 118),
        night: false,
    },
    Theme {
        name: "night",
        bg: (18, 18, 40),
        panel: (36, 36, 74),
        text: (240, 240, 255),
        dim: (150, 150, 195),
        accent: (255, 214, 0),
        answers: [(255, 95, 95), (64, 196, 255), (255, 215, 64), (105, 240, 174)],
        good: (105, 230, 150),
        bad: (255, 110, 110),
        sky: [(10, 10, 40), (58, 46, 110)],
        water: [(26, 35, 126), (63, 81, 181)],
        grass: (46, 100, 86),
        wood: (150, 118, 100),
        sun: (245, 245, 220),
        night: true,
    },
];

/// The names, for messages: "sunny, ocean, ...".
pub fn theme_names() -> String {
    THEMES.iter().map(|t| t.name).collect::<Vec<_>>().join(", ")
}

pub fn find_theme(name: &str) -> Option<usize> {
    THEMES.iter().position(|t| t.name.eq_ignore_ascii_case(name))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settings {
    pub theme: usize,
    /// Whether to look for a newer release when the game starts.
    pub update: bool,
    /// Whether things move, or simply appear where they belong.
    pub animations: bool,
    pub sound: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings { theme: 0, update: true, animations: true, sound: true }
    }
}

impl Settings {
    /// `settings` in the state directory.
    pub fn path() -> PathBuf {
        crate::update::state_dir().join("settings")
    }

    /// Lines of `name=value`. Anything missing or not understood keeps its default.
    pub fn parse(text: &str) -> Settings {
        let mut settings = Settings::default();
        for line in text.lines() {
            match line.split_once('=').map(|(k, v)| (k.trim(), v.trim())) {
                Some(("theme", v)) => settings.theme = find_theme(v).unwrap_or(settings.theme),
                Some(("update", v)) => settings.update = v != "0",
                Some(("animations", v)) => settings.animations = v != "0",
                Some(("sound", v)) => settings.sound = v != "0",
                _ => {}
            }
        }
        settings
    }

    pub fn format(&self) -> String {
        format!(
            "theme={}\nupdate={}\nanimations={}\nsound={}\n",
            THEMES[self.theme].name,
            u8::from(self.update),
            u8::from(self.animations),
            u8::from(self.sound)
        )
    }

    pub fn load(path: &std::path::Path) -> Settings {
        Settings::parse(&std::fs::read_to_string(path).unwrap_or_default())
    }

    /// Failing to save is not worth interrupting a game for.
    pub fn save(&self, path: &std::path::Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, self.format());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_survive_a_round_trip_and_bad_input() {
        let settings = Settings { theme: find_theme("ocean").unwrap(), update: false, animations: false, sound: false };
        assert_eq!(Settings::parse(&settings.format()), settings);
        assert_eq!(Settings::parse("theme=nope\nsound\n=\nanimations=0\nextra=1"), Settings { animations: false, ..Settings::default() });

        let dir = std::env::temp_dir().join(format!("fungeo-test-{}", std::process::id()));
        let path = dir.join("deeper/settings");
        assert_eq!(Settings::load(&path), Settings::default());
        settings.save(&path);
        assert_eq!(Settings::load(&path), settings);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn names_are_unique_and_found() {
        for (i, theme) in THEMES.iter().enumerate() {
            assert_eq!(find_theme(theme.name), Some(i));
        }
    }
}
