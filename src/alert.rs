//! The "time to switch!" side effects: chime and desktop notification.
//! (The on-widget flash is drawn in app.rs, because it is UI.)

use std::io::Cursor;

use crate::config::AlertStyle;
use crate::schedule::{Step, StepOptions, describe_duration};
use crate::timer::Mode;

// `include_bytes!` copies the file into the .exe at compile time, so there is
// nothing to install or lose. The path is relative to this source file.
const CHIME_STAND: &[u8] = include_bytes!("../assets/chime_stand.wav");
const CHIME_SIT: &[u8] = include_bytes!("../assets/chime_sit.wav");

pub struct Audio {
    /// The connection to the speakers. `None` if there is no audio device:
    /// the app should still work, just silently.
    device: Option<rodio::MixerDeviceSink>,
    /// The chime currently playing. Playback stops when a `Player` is dropped,
    /// so we keep it here instead of letting it go out of scope.
    current: Option<rodio::Player>,
}

impl Audio {
    pub fn new() -> Self {
        // `match` on a `Result`: `Ok(value)` on success, `Err(error)` on failure.
        let device = match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(mut device) => {
                device.log_on_drop(false);
                Some(device)
            }
            Err(err) => {
                eprintln!("No audio output, chimes disabled: {err}");
                None
            }
        };
        Audio {
            device,
            current: None,
        }
    }

    pub fn play_chime(&mut self, next: Mode, volume: f32) {
        // `let ... else` : bind the value if it's there, otherwise run the else block
        // (which must leave the function).
        let Some(device) = &self.device else {
            return;
        };
        let bytes = match next {
            Mode::Stand => CHIME_STAND,
            Mode::Sit => CHIME_SIT,
            // A break is time away from the desk: same "get up" chime as standing.
            Mode::Break => CHIME_STAND,
        };
        // `Cursor` makes an in-memory byte slice look like a file that can be read.
        match rodio::play(device.mixer(), Cursor::new(bytes)) {
            Ok(player) => {
                player.set_volume(volume);
                // Assigning replaces (and so stops) any chime that was still playing.
                self.current = Some(player);
            }
            Err(err) => eprintln!("Could not play chime: {err}"),
        }
    }
}

/// Which parts of an alert actually happen when an interval starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlertPlan {
    pub sound: bool,
    pub flash: bool,
    pub notification: bool,
}

impl AlertPlan {
    /// The alert style in effect (the schedule's or the app's), minus whatever the
    /// starting interval's `[...]` options turn off.
    pub fn new(style: AlertStyle, options: StepOptions) -> AlertPlan {
        AlertPlan {
            sound: style.sound() && !options.no_sound,
            flash: style.visual() && !options.no_flash,
            notification: style.notification() && !options.no_notification,
        }
    }
}

/// `next`: the interval that's starting. `following`: the one after it.
pub fn show_notification(next: &Step, following: &Step) {
    let (title, body) = notification_text(next, following);
    // Showing a toast talks to Windows and can take a moment, so do it on a
    // background thread to keep the widget responsive. `move` hands ownership of
    // `title` and `body` to the new thread.
    std::thread::spawn(move || {
        let mut notification = notify_rust::Notification::new();
        notification.summary(&title).body(&body);
        // Without this, Windows attributes the notification to PowerShell.
        #[cfg(target_os = "windows")]
        notification.app_id(crate::platform::APP_ID);
        let result = notification.show();
        if let Err(err) = result {
            eprintln!("Could not show notification: {err}");
        }
    });
}

/// Title and body, e.g. ("Long break", "Long break for 20 minutes, then sit.").
/// Schedules can use any label, so the title is the label itself; plain "sit" and
/// "stand" get a friendlier "Time to sit!".
fn notification_text(next: &Step, following: &Step) -> (String, String) {
    let title = match next.label.to_lowercase().as_str() {
        "sit" => "Time to sit!".to_owned(),
        "stand" => "Time to stand!".to_owned(),
        _ => capitalize(&next.label),
    };
    let body = format!(
        "{} for {}, then {}.",
        capitalize(&next.label),
        describe_duration(next.duration),
        following.label
    );
    (title, body)
}

/// "break" -> "Break".
fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::Schedule;

    fn text(schedule: &str) -> (String, String) {
        let schedule = Schedule::parse(schedule).unwrap();
        notification_text(schedule.step(0), schedule.step(1))
    }

    #[test]
    fn plain_sit_and_stand_get_friendly_titles() {
        assert_eq!(
            text("- 15m Stand\n- 30m sit"),
            (
                "Time to stand!".to_owned(),
                "Stand for 15 minutes, then sit.".to_owned()
            )
        );
    }

    #[test]
    fn step_options_only_turn_alert_parts_off() {
        let schedule = Schedule::parse("- 1m a [no sound]\n- 1m b [silent]\n- 1m c").unwrap();
        let plan = |i, style| AlertPlan::new(style, schedule.step(i).options);
        let all = AlertPlan {
            sound: true,
            flash: true,
            notification: true,
        };
        assert_eq!(
            plan(0, AlertStyle::All),
            AlertPlan {
                sound: false,
                ..all
            }
        );
        assert_eq!(
            plan(1, AlertStyle::All),
            AlertPlan {
                sound: false,
                flash: false,
                notification: false
            }
        );
        assert_eq!(plan(2, AlertStyle::All), all);
        // An option can't turn back on what the style leaves off.
        assert_eq!(
            plan(2, AlertStyle::SoundOnly),
            AlertPlan {
                sound: true,
                flash: false,
                notification: false
            }
        );
    }

    #[test]
    fn custom_labels_are_the_title() {
        assert_eq!(
            text("- 20m long break\n- 25m sit"),
            (
                "Long break".to_owned(),
                "Long break for 20 minutes, then sit.".to_owned()
            )
        );
    }
}
