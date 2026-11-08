//! User settings, saved as a TOML file in the OS config folder
//! (on Windows: `%APPDATA%\SitStand\config\config.toml`).

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::schedule::{self, Schedule};

// `Serialize`/`Deserialize` come from the serde crate: deriving them lets the
// `toml` crate convert this type to and from text automatically.
// `rename_all = "snake_case"` writes `VisualOnly` as "visual_only" in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertStyle {
    All,
    VisualOnly,
    SoundOnly,
    Silent,
}

impl AlertStyle {
    /// Every variant, in the order shown in the settings panel.
    pub const ALL_STYLES: [AlertStyle; 4] = [
        AlertStyle::All,
        AlertStyle::VisualOnly,
        AlertStyle::SoundOnly,
        AlertStyle::Silent,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AlertStyle::All => "Sound + visual + notification",
            AlertStyle::VisualOnly => "Visual only",
            AlertStyle::SoundOnly => "Sound only",
            AlertStyle::Silent => "Silent",
        }
    }

    pub fn sound(self) -> bool {
        // `matches!` is shorthand for a `match` that returns true/false.
        matches!(self, AlertStyle::All | AlertStyle::SoundOnly)
    }

    pub fn visual(self) -> bool {
        matches!(self, AlertStyle::All | AlertStyle::VisualOnly)
    }

    pub fn notification(self) -> bool {
        matches!(self, AlertStyle::All)
    }
}

/// How the widget shows an alert visually.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlashStyle {
    /// The whole background pulses toward the interval's color.
    WholeWidget,
    /// Only the colored border pulses, brighter and thicker.
    Border,
    /// A bright streak with a fading tail runs around the edge, in place of the border.
    Comet,
}

impl FlashStyle {
    /// Every variant, in the order shown in the settings panel.
    pub const ALL_STYLES: [FlashStyle; 3] = [
        FlashStyle::WholeWidget,
        FlashStyle::Border,
        FlashStyle::Comet,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FlashStyle::WholeWidget => "Whole widget",
            FlashStyle::Border => "Border",
            FlashStyle::Comet => "Comet",
        }
    }
}

// `#[serde(default)]`: any field missing from the file gets its value from
// `Config::default()`. So an old or hand-trimmed config file still loads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub sit_minutes: u32,
    pub stand_minutes: u32,
    /// Accent colors as [red, green, blue], 0-255.
    pub sit_color: [u8; 3],
    pub stand_color: [u8; 3],
    /// For schedule steps that aren't sit or stand (break, walk, ...).
    pub break_color: [u8; 3],
    /// Folder of `.sstc` schedules to choose from. Empty = `schedules` next to this config file.
    pub schedules_folder: String,
    /// File name (inside `schedules_folder`) of the schedule to follow. Empty = plain sit/stand.
    pub schedule: String,
    /// Background opacity, 0.0 (invisible) to 1.0 (solid). Text is always opaque.
    pub opacity: f32,
    /// Draw the colored border around the widget. (Border and Comet alerts still
    /// draw on the edge while they run.)
    pub show_border: bool,
    pub alert_style: AlertStyle,
    /// Chime volume, 0.0 to 1.0.
    pub volume: f32,
    pub muted: bool,
    /// `true`: the next interval starts by itself when one ends.
    /// `false`: it waits for a click ("confirm" mode), so it doesn't count while you're away.
    pub auto_start_next: bool,
    /// Repeat the chime if an alert isn't dismissed.
    pub repeat_reminder: bool,
    pub repeat_every_secs: u32,
    pub repeat_max_times: u32,
    /// How long the widget flashes when an interval starts (clicking it stops sooner).
    pub flash_secs: u32,
    pub flash_style: FlashStyle,
    /// Small pill showing only the time.
    pub compact: bool,
    /// Window position (top-left corner, in screen points). `None` = let the OS decide.
    pub window_pos: Option<[f32; 2]>,
    /// Look for a new release on GitHub once a day (installing still asks first).
    pub check_for_updates: bool,
    /// The pointer has been over the widget at least once, so the "Hover for
    /// controls" hint is no longer needed.
    pub hover_hint_seen: bool,
}

// `Default` is a standard trait: `Config::default()` gives the starting settings.
// Change the default durations here.
impl Default for Config {
    fn default() -> Self {
        Config {
            sit_minutes: 30,
            stand_minutes: 15,
            sit_color: [70, 140, 255],   // blue
            stand_color: [50, 205, 120], // green
            break_color: [240, 170, 60], // amber
            schedules_folder: String::new(),
            schedule: String::new(),
            opacity: 0.85,
            show_border: true,
            alert_style: AlertStyle::All,
            volume: 0.7,
            muted: false,
            auto_start_next: true,
            repeat_reminder: false,
            repeat_every_secs: 60,
            repeat_max_times: 3,
            flash_secs: 60,
            flash_style: FlashStyle::WholeWidget,
            compact: false,
            window_pos: None,
            check_for_updates: true,
            hover_hint_seen: false,
        }
    }
}

impl Config {
    pub fn sit_duration(&self) -> Duration {
        Duration::from_secs(u64::from(self.sit_minutes) * 60)
    }

    pub fn stand_duration(&self) -> Duration {
        Duration::from_secs(u64::from(self.stand_minutes) * 60)
    }

    /// The plain sit/stand cycle, from the two durations above.
    pub fn simple_schedule(&self) -> Schedule {
        Schedule::simple(self.sit_duration(), self.stand_duration())
    }

    /// Our folder for settings. `None` if the OS has no config folder for us.
    fn dir() -> Option<PathBuf> {
        let dirs = directories::ProjectDirs::from("", "", "SitStand")?;
        Some(dirs.config_dir().to_path_buf())
    }

    /// Where the config file lives.
    pub fn path() -> Option<PathBuf> {
        Some(Config::dir()?.join("config.toml"))
    }

    /// The schedules folder used when `schedules_folder` is left empty.
    pub fn default_schedules_folder() -> Option<PathBuf> {
        Some(Config::dir()?.join("schedules"))
    }

    pub fn schedules_folder_path(&self) -> Option<PathBuf> {
        let folder = self.schedules_folder.trim();
        if folder.is_empty() {
            Config::default_schedules_folder()
        } else {
            Some(PathBuf::from(folder))
        }
    }

    /// Creates the default schedules folder with an example in it, so a new user has
    /// a schedule to pick and one to copy. Called on the very first run only, so a
    /// folder the user deleted stays deleted.
    pub fn create_default_schedules_folder() {
        let Some(folder) = Config::default_schedules_folder() else {
            return;
        };
        let created = std::fs::create_dir_all(&folder)
            .and_then(|()| std::fs::write(folder.join("Pomodoro.sstc"), schedule::POMODORO));
        if let Err(err) = created {
            eprintln!("Could not create {}: {err}", folder.display());
        }
    }

    /// File names of the `.sstc` schedules in the schedules folder, sorted.
    /// The error is a message for the settings panel.
    pub fn list_schedules(&self) -> Result<Vec<String>, String> {
        let Some(folder) = self.schedules_folder_path() else {
            return Err("No config folder to keep schedules in.".to_owned());
        };
        // A missing default folder (deleted by the user) just means "no schedules".
        if self.schedules_folder.trim().is_empty() && !folder.exists() {
            return Ok(Vec::new());
        }
        let entries = std::fs::read_dir(&folder)
            .map_err(|err| format!("Could not open {}: {err}", folder.display()))?;
        // `filter_map` keeps only the entries for which the closure returns `Some`.
        let mut names: Vec<String> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("sstc"))
            })
            .filter_map(|path| Some(path.file_name()?.to_str()?.to_owned()))
            .collect();
        names.sort_by_key(|name| name.to_lowercase());
        Ok(names)
    }

    /// The schedule to follow: plain sit/stand, or the chosen file from the schedules folder.
    /// The error is a message for the settings panel.
    pub fn load_schedule(&self) -> Result<Schedule, String> {
        if self.schedule.is_empty() {
            return Ok(self.simple_schedule());
        }
        let Some(folder) = self.schedules_folder_path() else {
            return Err("No config folder to keep schedules in.".to_owned());
        };
        // Errors name just the file, not the whole path: they have to fit in the
        // narrow settings panel.
        let name = &self.schedule;
        let text = std::fs::read_to_string(folder.join(name))
            .map_err(|err| format!("Could not read {name}: {err}"))?;
        Schedule::parse(&text).map_err(|err| format!("{name}, {err}"))
    }

    /// Parse settings from TOML text, fixing any out-of-range values.
    pub fn from_toml_str(text: &str) -> Result<Config, toml::de::Error> {
        // `?` : if parsing fails, return the error from this function right away;
        // otherwise unwrap the `Ok` value and carry on.
        let config: Config = toml::from_str(text)?;
        Ok(config.sanitized())
    }

    /// Load settings from disk. Never fails: problems are logged and defaults used.
    pub fn load() -> Config {
        let Some(path) = Config::path() else {
            return Config::default();
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            // A missing file is normal on first run, so don't log that.
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Config::default(),
            Err(err) => {
                eprintln!("Could not read {}: {err}. Using defaults.", path.display());
                return Config::default();
            }
        };
        match Config::from_toml_str(&text) {
            Ok(config) => config,
            Err(err) => {
                // Keep the broken file (so hand edits aren't lost) and start fresh.
                let backup = path.with_extension("toml.bad");
                eprintln!("Invalid config file, moved to {}: {err}", backup.display());
                let _ = std::fs::rename(&path, &backup);
                Config::default()
            }
        }
    }

    /// Write settings to disk. Errors are logged, not fatal.
    pub fn save(&self) {
        let Some(path) = Config::path() else {
            return;
        };
        // A closure returning `Result` lets us use `?` for each step below and
        // handle any failure in one place.
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            // Write a temporary file, then swap it in with `rename` (which replaces the
            // old file in one step). A crash mid-write then leaves the old settings
            // intact, rather than a half-written file.
            let temp = path.with_extension("toml.tmp");
            std::fs::write(&temp, toml::to_string(self)?)?;
            std::fs::rename(&temp, &path)?;
            Ok(())
        })();
        if let Err(err) = result {
            eprintln!("Could not save settings to {}: {err}", path.display());
        }
    }

    /// Clamp values someone may have mistyped in the file into sensible ranges.
    fn sanitized(mut self) -> Config {
        self.sit_minutes = self.sit_minutes.clamp(1, 240);
        self.stand_minutes = self.stand_minutes.clamp(1, 240);
        self.opacity = self.opacity.clamp(0.0, 1.0);
        self.volume = self.volume.clamp(0.0, 1.0);
        self.repeat_every_secs = self.repeat_every_secs.clamp(10, 600);
        self.repeat_max_times = self.repeat_max_times.clamp(1, 20);
        self.flash_secs = self.flash_secs.clamp(1, 600);
        // NaN would never compare as "in range", so drop a position that isn't a real number.
        if self
            .window_pos
            .is_some_and(|[x, y]| !x.is_finite() || !y.is_finite())
        {
            self.window_pos = None;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_toml() {
        let config = Config {
            sit_minutes: 45,
            alert_style: AlertStyle::VisualOnly,
            window_pos: Some([100.0, 200.0]),
            ..Config::default()
        };
        let text = toml::to_string_pretty(&config).unwrap();
        assert_eq!(Config::from_toml_str(&text).unwrap(), config);
    }

    #[test]
    fn missing_fields_get_defaults() {
        let config = Config::from_toml_str("sit_minutes = 50").unwrap();
        assert_eq!(config.sit_minutes, 50);
        assert_eq!(config.stand_minutes, Config::default().stand_minutes);
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let config = Config::from_toml_str("sit_minutes = 0\nopacity = 7.5").unwrap();
        assert_eq!(config.sit_minutes, 1);
        assert_eq!(config.opacity, 1.0);
    }

    #[test]
    fn lists_and_loads_schedules_from_the_folder() {
        let folder = std::env::temp_dir().join(format!("sitstand-test-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("b.sstc"), "- 1m sit").unwrap();
        std::fs::write(folder.join("A.SSTC"), "- 2m stand").unwrap();
        std::fs::write(folder.join("broken.sstc"), "- 2 stand").unwrap();
        std::fs::write(folder.join("notes.txt"), "not a schedule").unwrap();
        let mut config = Config {
            schedules_folder: folder.display().to_string(),
            ..Config::default()
        };

        assert_eq!(
            config.list_schedules().unwrap(),
            ["A.SSTC", "b.sstc", "broken.sstc"]
        );
        config.schedule = "b.sstc".to_owned();
        assert_eq!(config.load_schedule().unwrap().step(0).label, "sit");
        config.schedule = "broken.sstc".to_owned();
        assert!(config.load_schedule().unwrap_err().contains("line 1"));
        config.schedule = "gone.sstc".to_owned();
        assert!(config.load_schedule().is_err());

        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn garbage_is_an_error_not_a_crash() {
        assert!(Config::from_toml_str("this is { not toml").is_err());
        assert!(Config::from_toml_str("sit_minutes = \"lots\"").is_err());
    }
}
