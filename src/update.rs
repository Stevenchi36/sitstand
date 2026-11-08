//! Checking GitHub for a newer release, and installing it.
//!
//! Each release carries a `latest.json` (version, installer URL, signature), made by
//! `.github/workflows/release.yml`. The updater crate makes blocking network calls, so
//! they run on background threads and report back over a channel; [`Updates::poll`],
//! called from `logic` every frame, picks up the results. The plan behind this is
//! `docs/auto-update-plan.md`.

use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use cargo_packager_updater::{Update, Updater, UpdaterBuilder};
use eframe::egui;

/// Where the newest release's `latest.json` lives. GitHub redirects
/// `releases/latest/download/...` to that file on the most recent release.
const ENDPOINT: &str =
    "https://github.com/Stevenchi36/sitstand/releases/latest/download/latest.json";

/// The public half of the release signing key (`sitstand.key.pub`). Downloads not
/// signed with the matching private key are refused, so a tampered `latest.json`
/// or installer can't get installed.
const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDI3ODNDNUU3NEIxMzgyQTUKUldTbGdoTkw1OFdESnlPQUNWWldVdTR1clllTXhpRzBqNFVYVHFKdXJUV2dQTm4xb1pzbUFsSnAK";

/// How often to look for a new release.
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
/// After a failed check (offline, GitHub down), try again sooner than a day.
const RETRY_AFTER: Duration = Duration::from_secs(60 * 60);
/// Give up on a request that takes longer than this.
const TIMEOUT: Duration = Duration::from_secs(30);

/// This build's version, from `Cargo.toml`. `env!` reads it at compile time.
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// What the background threads send back.
enum Message {
    /// A check finished: `Some` = a newer version exists.
    Checked(Result<Option<Box<Update>>, String>),
    /// Downloading or starting the installer failed. (When it works, the app has
    /// already exited, so there's no "success" message.)
    InstallFailed(String),
}

/// Where the updater is at.
enum State {
    NotChecked,
    Checking,
    UpToDate,
    CheckFailed(String),
    /// A newer version exists. `install_error` is why the last install attempt failed.
    // `Box`: `Update` is a large struct; boxing keeps every `State` small.
    Available {
        update: Box<Update>,
        install_error: Option<String>,
    },
    /// Downloading the installer (it then runs, and this app exits).
    Installing(Box<Update>),
}

/// Looks for updates in the background and installs one when asked.
pub struct Updates {
    state: State,
    next_check: Instant,
    sender: Sender<Message>,
    receiver: Receiver<Message>,
}

impl Updates {
    /// Starts with a check due right away.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Updates {
            state: State::NotChecked,
            next_check: Instant::now(),
            sender,
            receiver,
        }
    }

    /// Call every frame. Takes in finished background work, and starts a check when
    /// one is due (and `enabled`). Returns how long until the next check, for the
    /// app's `wake_in` list.
    pub fn poll(&mut self, ctx: &egui::Context, now: Instant, enabled: bool) -> Option<Duration> {
        // Collect first: handling a message changes `self`, which can't happen
        // while `self.receiver` is still borrowed by the iterator.
        let messages: Vec<Message> = self.receiver.try_iter().collect();
        for message in messages {
            self.handle(message, now);
        }
        if !enabled || !self.can_check() {
            return None;
        }
        if now >= self.next_check {
            self.check_now(ctx);
            return None;
        }
        Some(self.next_check - now)
    }

    fn handle(&mut self, message: Message, now: Instant) {
        match message {
            Message::Checked(Ok(Some(update))) => {
                self.state = State::Available {
                    update,
                    install_error: None,
                };
            }
            Message::Checked(Ok(None)) => {
                self.state = State::UpToDate;
                self.next_check = now + CHECK_EVERY;
            }
            Message::Checked(Err(err)) => {
                self.state = State::CheckFailed(err);
                self.next_check = now + RETRY_AFTER;
            }
            Message::InstallFailed(err) => {
                // Offer the same update again, with the reason it failed.
                if let State::Installing(update) = &self.state {
                    self.state = State::Available {
                        update: update.clone(),
                        install_error: Some(err),
                    };
                }
            }
        }
    }

    /// True when no check is running and no update is waiting.
    pub fn can_check(&self) -> bool {
        matches!(
            self.state,
            State::NotChecked | State::UpToDate | State::CheckFailed(_)
        )
    }

    /// Looks for a new release now, on a background thread.
    pub fn check_now(&mut self, ctx: &egui::Context) {
        if !self.can_check() {
            return;
        }
        self.state = State::Checking;
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            // `.and_then` runs `check` only if building the updater worked;
            // `.map` boxes a found update; `.map_err` turns an error into its message.
            let result = updater()
                .and_then(|u| u.check())
                .map(|found| found.map(Box::new))
                .map_err(|e| e.to_string());
            let _ = sender.send(Message::Checked(result));
            ctx.request_repaint();
        });
    }

    /// The newer version on offer (also while it's being installed).
    pub fn available_version(&self) -> Option<&str> {
        match &self.state {
            State::Available { update, .. } | State::Installing(update) => Some(&update.version),
            _ => None,
        }
    }

    pub fn is_installing(&self) -> bool {
        matches!(self.state, State::Installing(_))
    }

    /// Downloads the update and runs its installer, on a background thread. If that
    /// works, the process exits (the installer restarts the app afterwards), so
    /// **save anything unsaved before calling this**.
    pub fn install(&mut self, ctx: &egui::Context) {
        let State::Available { update, .. } = &self.state else {
            return;
        };
        let update = update.clone();
        self.state = State::Installing(update.clone());
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            // Checks the signature before running anything. On success this never
            // returns: the crate starts the installer and ends our process.
            if let Err(err) = update.download_and_install() {
                let _ = sender.send(Message::InstallFailed(err.to_string()));
                ctx.request_repaint();
            }
        });
    }

    /// One line for the settings panel.
    pub fn status_text(&self) -> String {
        match &self.state {
            State::NotChecked => format!("Version {CURRENT_VERSION}"),
            State::Checking => "Checking for updates…".to_owned(),
            State::UpToDate => format!("Up to date ({CURRENT_VERSION})"),
            State::CheckFailed(_) => "Couldn't check for updates".to_owned(),
            State::Available {
                update,
                install_error: None,
            } => format!("Version {} is available", update.version),
            State::Available {
                install_error: Some(err),
                ..
            } => format!("Update failed: {err}"),
            State::Installing(_) => "Downloading update…".to_owned(),
        }
    }

    /// Why the last check failed, for a hover tooltip.
    pub fn check_error(&self) -> Option<&str> {
        match &self.state {
            State::CheckFailed(err) => Some(err),
            _ => None,
        }
    }
}

/// An updater pointed at our releases, for this version.
fn updater() -> cargo_packager_updater::Result<Updater> {
    let config = cargo_packager_updater::Config {
        // `.expect` is fine here: these are constants, checked by the tests below.
        endpoints: vec![ENDPOINT.parse().expect("ENDPOINT is a valid URL")],
        pubkey: PUBLIC_KEY.to_owned(),
        // `..Default::default()`: every other field takes its default value
        // (on Windows: run the installer with a progress bar only, then restart the app).
        ..Default::default()
    };
    let version = CURRENT_VERSION
        .parse()
        .expect("Cargo.toml version is valid semver");
    UpdaterBuilder::new(version, config)
        .timeout(TIMEOUT)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updater_builds_from_the_constants() {
        // Parses ENDPOINT and CURRENT_VERSION; no network involved.
        assert!(updater().is_ok());
    }

    #[test]
    fn a_failed_check_retries_sooner_than_a_successful_one() {
        let mut updates = Updates::new();
        let t0 = Instant::now();
        updates.handle(Message::Checked(Err("offline".into())), t0);
        assert_eq!(updates.next_check, t0 + RETRY_AFTER);
        assert!(updates.can_check());
        assert_eq!(updates.check_error(), Some("offline"));

        updates.handle(Message::Checked(Ok(None)), t0);
        assert_eq!(updates.next_check, t0 + CHECK_EVERY);
        assert_eq!(
            updates.status_text(),
            format!("Up to date ({CURRENT_VERSION})")
        );
    }
}
