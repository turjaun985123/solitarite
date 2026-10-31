use std::{
    env, fs,
    path::{Path, PathBuf},
};

use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::model::{Game, Rules, Scoring, format_time};

pub const SAVE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    Green,
    Midnight,
    Burgundy,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::Green, Theme::Midnight, Theme::Burgundy];

    pub fn label(self) -> &'static str {
        match self {
            Theme::Green => "Green Felt",
            Theme::Midnight => "Midnight",
            Theme::Burgundy => "Burgundy",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Theme::Green => Theme::Midnight,
            Theme::Midnight => Theme::Burgundy,
            Theme::Burgundy => Theme::Green,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardBack {
    Midnight,
    Crimson,
    Emerald,
    Royal,
}

impl CardBack {
    pub const ALL: [CardBack; 4] = [
        CardBack::Midnight,
        CardBack::Crimson,
        CardBack::Emerald,
        CardBack::Royal,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CardBack::Midnight => "Midnight",
            CardBack::Crimson => "Crimson",
            CardBack::Emerald => "Emerald",
            CardBack::Royal => "Royal",
        }
    }

    pub fn next(self) -> Self {
        match self {
            CardBack::Midnight => CardBack::Crimson,
            CardBack::Crimson => CardBack::Emerald,
            CardBack::Emerald => CardBack::Royal,
            CardBack::Royal => CardBack::Midnight,
        }
    }
}

fn default_sound_effects() -> bool {
    true
}

fn default_volume() -> f32 {
    0.8
}

fn default_animations() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub rules: Rules,
    pub auto_play: bool,
    pub theme: Theme,
    pub card_back: CardBack,
    /// Play short UI/game sounds when an audio output device is available.
    #[serde(default = "default_sound_effects")]
    pub sound_effects: bool,
    /// Master volume for sound effects, 0.0 (silent) to 1.0 (full).
    #[serde(default = "default_volume")]
    pub volume: f32,
    /// Animate cards travelling between piles, flipping and dealing.
    #[serde(default = "default_animations")]
    pub animations: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            rules: Rules::default(),
            auto_play: true,
            theme: Theme::Green,
            card_back: CardBack::Midnight,
            sound_effects: true,
            volume: default_volume(),
            animations: true,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub games_played: u64,
    pub games_won: u64,
    pub current_streak: u32,
    pub best_streak: u32,
    pub best_time: Option<u64>,
    pub fewest_moves: Option<u32>,
    pub best_standard_score: Option<i64>,
    pub best_vegas_score: Option<i64>,
    pub total_time_won: u64,
    pub last_played: Option<String>,
}

impl Stats {
    pub fn win_rate(&self) -> f32 {
        if self.games_played == 0 {
            0.0
        } else {
            self.games_won as f32 / self.games_played as f32
        }
    }

    pub fn win_rate_label(&self) -> String {
        format!("{:.0}%", self.win_rate() * 100.0)
    }

    pub fn record_game_started(&mut self) {
        self.games_played += 1;
        self.last_played = Some(Local::now().format("%b %-d, %Y").to_string());
    }

    pub fn abandon(&mut self, game: &Game) {
        if !game.won && game.moves > 0 {
            self.current_streak = 0;
        }
    }

    pub fn record_win(&mut self, game: &Game) {
        self.games_won += 1;
        self.current_streak += 1;
        self.best_streak = self.best_streak.max(self.current_streak);
        self.best_time = Some(
            self.best_time
                .map_or(game.elapsed, |best| best.min(game.elapsed)),
        );
        self.fewest_moves = Some(
            self.fewest_moves
                .map_or(game.moves, |best| best.min(game.moves)),
        );
        match game.rules.scoring {
            Scoring::Standard => {
                self.best_standard_score = Some(
                    self.best_standard_score
                        .map_or(game.score, |best| best.max(game.score)),
                );
            }
            Scoring::Vegas => {
                self.best_vegas_score = Some(
                    self.best_vegas_score
                        .map_or(game.score, |best| best.max(game.score)),
                );
            }
        }
        self.total_time_won += game.elapsed;
        self.last_played = Some(Local::now().format("%b %-d, %Y").to_string());
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn best_time_label(&self) -> String {
        self.best_time
            .map(format_time)
            .unwrap_or_else(|| "—".into())
    }

    pub fn total_time_label(&self) -> String {
        format_time(self.total_time_won)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveData {
    pub version: u32,
    pub settings: Settings,
    pub stats: Stats,
    pub game: Option<Game>,
}

impl Default for SaveData {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            settings: Settings::default(),
            stats: Stats::default(),
            game: None,
        }
    }
}

pub fn data_dir() -> PathBuf {
    if let Ok(path) = env::var("SOLITARITE_DATA_DIR") {
        return PathBuf::from(path);
    }
    if let Ok(path) = env::var("XDG_DATA_HOME") {
        return PathBuf::from(path).join("solitarite");
    }
    if let Ok(home) = env::var("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("solitarite");
    }
    PathBuf::from(".solitarite")
}

pub fn save_path() -> PathBuf {
    data_dir().join("save.json")
}

pub fn save_path_display() -> String {
    let path = save_path();
    if let Ok(home) = env::var("HOME")
        && let Ok(relative) = path.strip_prefix(&home)
    {
        return format!("~/{}", relative.display());
    }
    path.display().to_string()
}

pub fn load() -> SaveData {
    let path = save_path();
    let Ok(text) = fs::read_to_string(&path) else {
        return SaveData::default();
    };
    match serde_json::from_str::<SaveData>(&text) {
        Ok(data) if data.version <= SAVE_VERSION => data,
        Ok(_) => {
            eprintln!("save file has a newer version; starting fresh");
            SaveData::default()
        }
        Err(error) => {
            eprintln!("could not read save file: {error}");
            let _ = fs::rename(&path, path.with_extension("json.corrupt"));
            SaveData::default()
        }
    }
}

pub fn save(data: &SaveData) -> std::io::Result<()> {
    let dir = data_dir();
    fs::create_dir_all(&dir)?;
    let path = dir.join("save.json");
    let temp = dir.join("save.json.tmp");
    let text = serde_json::to_string_pretty(data)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(&temp, text)?;
    replace_file(&temp, &path)?;
    Ok(())
}

fn replace_file(from: &Path, to: &Path) -> std::io::Result<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            let _ = fs::remove_file(to);
            fs::rename(from, to)
        }
    }
}

#[cfg(test)]
mod settings_tests {
    use super::Settings;

    #[test]
    fn older_settings_default_sound_effects_to_enabled() {
        let old_settings = r#"{
            "rules": { "draw_three": false, "scoring": "Standard", "timed": false },
            "auto_play": true,
            "theme": "Green",
            "card_back": "Midnight"
        }"#;

        let settings: Settings = serde_json::from_str(old_settings).unwrap();
        assert!(settings.sound_effects);
    }

    #[test]
    fn older_settings_get_default_volume_and_animations() {
        let old_settings = r#"{
            "rules": {"draw_three": false, "scoring": "Standard", "timed": true},
            "auto_play": true,
            "theme": "Green",
            "card_back": "Midnight"
        }"#;
        let settings: Settings = serde_json::from_str(old_settings).unwrap();
        assert!((settings.volume - 0.8).abs() < f32::EPSILON);
        assert!(settings.animations);
    }

    #[test]
    fn sound_effect_setting_can_be_disabled() {
        let mut settings = Settings::default();
        settings.sound_effects = false;
        let encoded = serde_json::to_string(&settings).unwrap();
        let decoded: Settings = serde_json::from_str(&encoded).unwrap();
        assert!(!decoded.sound_effects);
    }
}
