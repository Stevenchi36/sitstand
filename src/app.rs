//! The egui app: draws the widget, handles clicks, and reacts to timer events.

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{
    Align2, Color32, FontFamily, FontId, Pos2, RichText, Stroke, StrokeKind, ViewportCommand, vec2,
};

use crate::alert::{self, AlertPlan, Audio};
use crate::config::{AlertStyle, Config, FlashStyle};
use crate::hotkey::{self, ClickThroughHotkey};
use crate::schedule::{self, Reminder, Schedule, Step, describe_duration};
use crate::single_instance::SingleInstance;
use crate::timer::{Mode, Timer, TimerEvent};
use crate::tray::{Tray, TrayAction, TrayState};
use crate::update::Updates;

// Window sizes, as [width, height] in points.
const MAIN_SIZE: [f32; 2] = [140.0, 140.0];
const COMPACT_SIZE: [f32; 2] = [120.0, 44.0];
const SETTINGS_SIZE: [f32; 2] = [220.0, 638.0];

// Look. The per-mode accent colors are settings (see config.rs).
const BG_COLOR: Color32 = Color32::from_rgb(24, 26, 31);
const CORNER_RADIUS: f32 = 12.0;
const RING_WIDTH: f32 = 6.0;

/// Settings are written to disk at most this often.
const SAVE_DELAY: Duration = Duration::from_secs(1);
/// How often to look for schedule files added to (or removed from) the folder, so
/// the tray's Schedule menu stays current. Listing a small folder is cheap.
const SCHEDULE_RESCAN: Duration = Duration::from_secs(5);

/// The window size for the current view. Also used by main.rs for the first frame.
pub fn window_size(config: &Config, settings_open: bool) -> [f32; 2] {
    if settings_open {
        SETTINGS_SIZE
    } else if config.compact {
        COMPACT_SIZE
    } else {
        MAIN_SIZE
    }
}

/// Tooltips can't draw outside our small window, and egui lets them grow to 500
/// points wide, so long ones got cut off. Wrap them to the window's width instead.
fn fit_tooltips(ctx: &egui::Context, window: [f32; 2]) {
    ctx.all_styles_mut(|style| style.spacing.tooltip_width = window[0] - 20.0);
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Mix two colors: t = 0.0 gives `a`, t = 1.0 gives `b`.
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(lerp(a.r(), b.r()), lerp(a.g(), b.g()), lerp(a.b(), b.b()))
}

fn digits_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("digits".into()))
}

/// Which picture an [`icon_button`] shows.
#[derive(Clone, Copy)]
enum Icon {
    Play,
    Pause,
    Skip,
    Gear,
}

/// A small clickable icon at a fixed spot (`rect`), drawn with lines and shapes
/// (the built-in fonts have no gear or media-control glyphs).
fn icon_button(ui: &egui::Ui, rect: egui::Rect, icon: Icon) -> egui::Response {
    // `ui.interact` makes an existing rect clickable without laying anything out.
    // The id only has to be unique per button, so the rect's position is enough.
    let response = ui.interact(
        rect,
        ui.scope_id().with(("icon", rect.min.x as i32)),
        egui::Sense::click(),
    );
    let color = if response.hovered() {
        Color32::WHITE
    } else {
        Color32::GRAY
    };
    let c = rect.center();
    let painter = ui.painter();
    // A right-pointing triangle, `w` wide, centered on `x`.
    let triangle = |x: f32, w: f32| {
        egui::Shape::convex_polygon(
            vec![
                egui::pos2(x - w / 2.0, c.y - 6.0),
                egui::pos2(x + w / 2.0, c.y),
                egui::pos2(x - w / 2.0, c.y + 6.0),
            ],
            color,
            Stroke::NONE,
        )
    };
    let bar = |x: f32| egui::Rect::from_center_size(egui::pos2(x, c.y), vec2(3.0, 12.0));
    match icon {
        Icon::Play => {
            painter.add(triangle(c.x + 1.0, 10.0));
        }
        Icon::Pause => {
            painter.rect_filled(bar(c.x - 3.0), 1.0, color);
            painter.rect_filled(bar(c.x + 3.0), 1.0, color);
        }
        Icon::Skip => {
            painter.add(triangle(c.x - 2.0, 9.0));
            painter.rect_filled(bar(c.x + 4.0), 1.0, color);
        }
        Icon::Gear => {
            for i in 0..8 {
                let angle = i as f32 * TAU / 8.0;
                let dir = vec2(angle.cos(), angle.sin());
                painter.line_segment([c + dir * 4.0, c + dir * 7.0], Stroke::new(2.5, color));
            }
            painter.circle_stroke(c, 4.5, Stroke::new(2.0, color));
        }
    }
    response
}

/// Paints one line of text centered on `center`, cut short with "…" if it's
/// wider than `max_width`.
fn paint_fitted(
    painter: &egui::Painter,
    center: Pos2,
    text: String,
    font: FontId,
    color: Color32,
    max_width: f32,
) {
    // A `LayoutJob` describes text to lay out; its `wrap` settings can limit it
    // to one row of a given width, ending in "…".
    let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_width);
    let galley = painter.layout_job(job);
    // `galley.size()` is the laid-out text's width and height.
    painter.galley(center - galley.size() / 2.0, galley, color);
}

/// Starts a section of the settings panel: a divider line (unless it's the first
/// section) and a small gray title. An empty title gives just the divider.
fn section_heading(ui: &mut egui::Ui, title: &str, divider: bool) {
    if divider {
        ui.add_space(8.0);
        ui.separator();
    }
    ui.add_space(4.0);
    if !title.is_empty() {
        ui.label(
            RichText::new(title)
                .small()
                .strong()
                .color(Color32::from_gray(140)),
        );
        ui.add_space(2.0);
    }
}

/// Shown in the settings in place of a control, when the loaded schedule file sets
/// that setting itself: what it's set to.
fn set_by_schedule(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(format!("{text} (set by schedule)")).weak())
        .on_hover_text(
            "The schedule file sets this in its ***** block.\n\
             Edit the file, or pick Plain sit/stand, to change it.",
        );
}

/// Brightness of an alert's pulse, `t` seconds in: 0 → 1 → 0, a bit over once a second.
fn pulse(t: f32) -> f32 {
    0.5 - 0.5 * (t * TAU * 1.2).cos()
}

/// Points spaced `step` apart along the outline of a rounded rectangle, going
/// clockwise. Used to run the "Comet" alert around the edge.
fn rounded_rect_outline(rect: egui::Rect, radius: f32, step: f32) -> Vec<Pos2> {
    // First a fine outline: each corner's quarter circle, clockwise from the top
    // right. The straight edges are the gaps between one arc's end and the next start.
    // Screen y points down, so a growing angle turns clockwise.
    let corners = [
        (
            egui::pos2(rect.right() - radius, rect.top() + radius),
            -FRAC_PI_2,
        ),
        (
            egui::pos2(rect.right() - radius, rect.bottom() - radius),
            0.0,
        ),
        (
            egui::pos2(rect.left() + radius, rect.bottom() - radius),
            FRAC_PI_2,
        ),
        (egui::pos2(rect.left() + radius, rect.top() + radius), PI),
    ];
    let mut outline = Vec::new();
    for (center, start) in corners {
        for i in 0..=8 {
            let angle = start + FRAC_PI_2 * i as f32 / 8.0;
            outline.push(center + radius * vec2(angle.cos(), angle.sin()));
        }
    }
    outline.push(outline[0]); // close the loop

    // Then walk it, dropping a point every `step`, so the comet moves at an even speed.
    let mut points = vec![outline[0]];
    let mut since_last = 0.0; // distance walked since the last point
    // `windows(2)` gives each pair of neighbours: [a, b], [b, c], ...
    for pair in outline.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let length = a.distance(b);
        let mut along = step - since_last;
        while along <= length {
            points.push(a + (b - a) * (along / length));
            along += step;
        }
        since_last = length - (along - step);
    }
    points
}

/// The "Comet" alert: a bright streak with a fading tail running around the edge,
/// once every `LAP_SECS`. `t` is seconds since the alert started.
fn draw_comet(painter: &egui::Painter, rect: egui::Rect, color: Color32, t: f32) {
    const LAP_SECS: f32 = 1.6;
    // Run along the middle of the 1.5-wide border line, so the streak takes the
    // border's place. (The outer half of the faint halo falls outside the window.)
    let inset = 0.75;
    let outline = rounded_rect_outline(rect.shrink(inset), CORNER_RADIUS - inset, 2.0);
    let n = outline.len();
    let head = ((t / LAP_SECS).fract() * n as f32) as usize;
    let tail = n / 4; // a quarter of the way around
    for k in 0..tail {
        // 1.0 at the head, fading to 0 at the end of the tail.
        let fade = 1.0 - k as f32 / tail as f32;
        // `+ n` before `-` keeps the index from going below zero.
        let i = (head + n - k) % n;
        let segment = [outline[(i + n - 1) % n], outline[i]];
        // A wide, faint stroke under a narrow, bright one reads as a glow.
        painter.line_segment(
            segment,
            Stroke::new(6.0 * fade, color.gamma_multiply(0.25 * fade)),
        );
        painter.line_segment(segment, Stroke::new(2.5, color.gamma_multiply(fade)));
    }
}

/// Progress ring: a faint full circle, with the time that's left drawn on top
/// as an arc starting at 12 o'clock. The arc shrinks as the interval runs down.
fn draw_ring(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    fraction_left: f32,
    color: Color32,
) {
    painter.circle_stroke(
        center,
        radius,
        Stroke::new(RING_WIDTH, Color32::from_white_alpha(20)),
    );
    if fraction_left <= 0.0 {
        return;
    }
    // An arc is drawn as many short straight lines; 64 per full circle looks smooth.
    let segments = (64.0 * fraction_left).ceil() as usize;
    // `(0..=segments).map(...).collect()` builds a Vec by running the closure for each number.
    let points: Vec<Pos2> = (0..=segments)
        .map(|i| {
            // Screen y points down, so a growing angle goes clockwise.
            let angle = -FRAC_PI_2 + TAU * fraction_left * (i as f32 / segments as f32);
            center + radius * vec2(angle.cos(), angle.sin())
        })
        .collect();
    painter.add(egui::Shape::line(points, Stroke::new(RING_WIDTH, color)));
}

/// Formats a duration as MM:SS, rounding *up* so a fresh 30-minute timer shows
/// 30:00 and the last second shows 00:01 rather than 00:00.
fn format_mm_ss(d: Duration) -> String {
    let total_secs = d.as_millis().div_ceil(1000);
    format!("{:02}:{:02}", total_secs / 60, total_secs % 60)
}

/// The schedule the settings ask for. If the schedule file has a problem, falls
/// back to plain sit/stand and returns the problem too, to show in the settings.
fn schedule_or_fallback(config: &Config) -> (Schedule, Option<String>) {
    match config.load_schedule() {
        Ok(schedule) => (schedule, None),
        Err(err) => {
            eprintln!("Schedule not loaded: {err}");
            (config.simple_schedule(), Some(err))
        }
    }
}

/// An alert that hasn't been acknowledged yet.
struct ActiveAlert {
    mode: Mode,
    /// The color to flash: the new interval's.
    color: [u8; 3],
    started: Instant,
    /// Whether to flash the widget, and whether it chimed (so reminder chimes may
    /// follow). Both depend on the alert style and the interval's `[...]` options.
    flash: bool,
    sound: bool,
    /// How long to flash if nobody clicks the widget.
    flash_for: Duration,
    /// How many extra "reminder" chimes have played so far.
    repeats_done: u32,
}

pub struct SitStandApp {
    config: Config,
    /// The settings as last written to disk, to tell whether we need to save.
    saved_config: Config,
    last_save: Instant,
    timer: Timer,
    /// Why the schedule file couldn't be used (plain sit/stand runs instead).
    schedule_error: Option<String>,
    /// The `.sstc` files in the schedules folder, as of the last look (every
    /// SCHEDULE_RESCAN, and on Reload). `Err`: the folder couldn't be read.
    schedule_files: Result<Vec<String>, String>,
    last_schedule_scan: Instant,
    audio: Audio,
    alert: Option<ActiveAlert>,
    /// In "wait for my click" mode: an interval ended and the next one is waiting to start.
    awaiting_click: bool,
    /// When a click on the widget last started a waiting interval. If that click turns
    /// out to be the first half of a double-click (toggle compact), the start is undone.
    click_started_at: Option<Instant>,
    /// The "Other folder" text box while it's being edited. Copied into
    /// `config.schedules_folder` only when the box loses focus, so a half-typed path
    /// is never used.
    folder_draft: String,
    /// `Some` while the "choose a folder" dialog is open: the folder picked
    /// arrives here (see `browse_for_folder`).
    folder_pick: Option<std::sync::mpsc::Receiver<std::path::PathBuf>>,
    show_settings: bool,
    /// Mouse clicks go "through" the widget to whatever is underneath.
    /// Deliberately not a saved setting, so the app never starts up unclickable.
    click_through: bool,
    /// `None` if the shortcut couldn't be registered.
    hotkey: Option<ClickThroughHotkey>,
    /// `None` if the tray icon couldn't be created.
    tray: Option<Tray>,
    /// False while hidden via the tray menu.
    visible: bool,
    /// Signals from later launches of the app, asking this copy to show itself.
    show_requests: std::sync::mpsc::Receiver<()>,
    /// Looks for new releases, and installs one when asked.
    updates: Updates,
    /// Why the "Start with Windows" entry couldn't be written, if it couldn't.
    startup_error: Option<String>,
}

impl SitStandApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config, instance: SingleInstance) -> Self {
        // Register the bundled font under the family name "digits".
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "jetbrains_mono".to_owned(),
            Arc::new(egui::FontData::from_static(include_bytes!(
                "../assets/JetBrainsMono-Bold.ttf"
            ))),
        );
        fonts.families.insert(
            FontFamily::Name("digits".into()),
            vec!["jetbrains_mono".to_owned()],
        );
        cc.egui_ctx.set_fonts(fonts);
        crate::platform::remove_os_window_frame(cc);
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        // Labels are text-selectable by default, which would swallow clicks and drags
        // on the digits. We want those to reach the window (drag / click to start).
        cc.egui_ctx
            .all_styles_mut(|style| style.interaction.selectable_labels = false);
        fit_tooltips(&cc.egui_ctx, window_size(&config, false));

        let (schedule, schedule_error) = schedule_or_fallback(&config);
        let schedule_files = config.list_schedules();
        let mut timer = Timer::new(schedule);
        timer.set_auto_continue(config.auto_start_next);
        // `let mut app = ...; ...; app`: build the struct, adjust it, then return it.
        let mut app = SitStandApp {
            // `config.clone()` makes a separate copy; afterwards `config` itself is
            // *moved* into the struct (no copy), and can't be used here any more.
            saved_config: config.clone(),
            folder_draft: config.schedules_folder.clone(),
            folder_pick: None,
            config,
            last_save: Instant::now(),
            timer,
            schedule_error,
            schedule_files,
            last_schedule_scan: Instant::now(),
            audio: Audio::new(),
            alert: None,
            awaiting_click: false,
            click_started_at: None,
            show_settings: false,
            click_through: false,
            hotkey: ClickThroughHotkey::register(&cc.egui_ctx),
            tray: Tray::new(&cc.egui_ctx),
            visible: true,
            show_requests: instance.listen(&cc.egui_ctx),
            updates: Updates::new(),
            startup_error: None,
        };
        // Write the startup entry again in case the program has moved (e.g. a new
        // install folder), so it always points at this copy.
        app.apply_start_with_windows();
        app
    }

    /// Makes Windows' startup list match the "Start with Windows" setting.
    fn apply_start_with_windows(&mut self) {
        // A debug build would register `target\debug\sitstand.exe` to run at
        // sign-in, which nobody wants. `cfg!` is true/false at compile time.
        if cfg!(debug_assertions) {
            eprintln!("Debug build: leaving Windows startup unchanged");
            return;
        }
        // `.err()` keeps the error message, if any, and drops the `Ok`.
        self.startup_error =
            crate::platform::set_start_with_windows(self.config.start_with_windows).err();
    }

    /// An interval's color: its own from the schedule file's ***** block if it has
    /// one, otherwise the settings-panel color for its mode.
    fn step_rgb(&self, step: &Step) -> [u8; 3] {
        step.color.unwrap_or(match step.mode {
            Mode::Sit => self.config.sit_color,
            Mode::Stand => self.config.stand_color,
            Mode::Break => self.config.break_color,
        })
    }

    /// The current interval's color.
    fn accent(&self) -> Color32 {
        rgb(self.step_rgb(self.timer.step()))
    }

    /// Re-read the schedule after its settings (or the file) changed. Starts the new
    /// schedule from its first step, unless it's the same as the one running.
    fn reload_schedule(&mut self, now: Instant) {
        let (schedule, error) = schedule_or_fallback(&self.config);
        self.schedule_error = error;
        if &schedule != self.timer.schedule() {
            self.timer.set_schedule(schedule);
            self.timer.restart_schedule(now);
            self.alert = None;
        }
    }

    /// Look again for schedule files in the folder.
    fn rescan_schedules(&mut self) {
        self.schedule_files = self.config.list_schedules();
        self.last_schedule_scan = Instant::now();
    }

    /// Switch to another schedules folder (empty = the default one).
    fn set_schedules_folder(&mut self, folder: String, now: Instant) {
        self.folder_draft.clone_from(&folder);
        self.config.schedules_folder = folder;
        self.rescan_schedules();
        // The schedule picked from the old folder may not be in the new one:
        // go back to plain sit/stand rather than show a "could not read" error.
        let still_there = self
            .schedule_files
            .as_ref()
            .is_ok_and(|names| names.contains(&self.config.schedule));
        if !still_there {
            self.config.schedule.clear();
        }
        self.reload_schedule(now);
    }

    /// Downloads and installs the available update. When that works the process
    /// exits straight away (eframe's `on_exit` doesn't run), so save settings first.
    fn install_update(&mut self, ctx: &egui::Context) {
        self.config.save();
        self.saved_config = self.config.clone();
        self.updates.install(ctx);
    }

    /// Opens the system's "choose a folder" dialog for the schedules folder.
    /// The dialog runs on its own thread so the timer keeps ticking while it's
    /// open; the chosen folder comes back through `folder_pick`.
    fn browse_for_folder(&mut self, ctx: &egui::Context) {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.folder_pick = Some(receiver);
        let start_in = self.config.schedules_folder_path();
        // `Context` is cheap to clone (it's a shared handle), and the clone can be
        // moved into the thread.
        let ctx = ctx.clone();
        // `move` makes the closure own `sender`, `start_in` and `ctx`.
        std::thread::spawn(move || {
            let mut dialog = rfd::FileDialog::new().set_title("Schedules folder");
            if let Some(folder) = start_in {
                dialog = dialog.set_directory(folder);
            }
            // Blocks this thread until the dialog closes. `None` = cancelled; the
            // sender is then dropped without sending, which `logic` notices.
            if let Some(folder) = dialog.pick_folder() {
                let _ = sender.send(folder);
            }
            ctx.request_repaint();
        });
    }

    /// The alert layer: decides what to do when the timer says an interval ended.
    /// `next` is the interval starting now; `following` is the one after it.
    fn fire_alert(&mut self, next: &Step, following: &Step, now: Instant) {
        let plan = AlertPlan::new(self.alert_style(), next.options);
        if plan.sound && !self.config.muted {
            self.audio.play_chime(next.mode, self.config.volume);
        }
        if plan.notification {
            alert::show_notification(next, following);
        }
        if plan.flash || plan.sound {
            self.alert = Some(ActiveAlert {
                mode: next.mode,
                color: self.step_rgb(next),
                started: now,
                flash: plan.flash,
                sound: plan.sound,
                // Never longer than the interval itself (a 30-second stretch, say).
                flash_for: self.flash_duration().min(next.duration),
                repeats_done: 0,
            });
        }
    }

    /// The alert style in effect: the schedule file's, if it sets one.
    fn alert_style(&self) -> AlertStyle {
        self.timer
            .schedule()
            .settings()
            .alerts
            .unwrap_or(self.config.alert_style)
    }

    /// How long an alert flashes: the schedule file's `flash`, if it sets one.
    fn flash_duration(&self) -> Duration {
        self.timer
            .schedule()
            .settings()
            .flash
            .unwrap_or(Duration::from_secs(u64::from(self.config.flash_secs)))
    }

    /// The repeat chime in effect, if any: the schedule file's, if it sets one.
    fn reminder(&self) -> Option<Reminder> {
        // The file's setting wins when it has one; `Some(None)` there means "off".
        if let Some(from_file) = self.timer.schedule().settings().reminder {
            return from_file;
        }
        let c = &self.config;
        // `bool::then_some`: `Some(value)` if true, `None` if false.
        c.repeat_reminder.then_some(Reminder {
            every_secs: c.repeat_every_secs,
            max_times: c.repeat_max_times,
        })
    }

    fn is_flashing(&self, now: Instant) -> bool {
        self.alert
            .as_ref()
            .is_some_and(|a| a.flash && now.duration_since(a.started) < a.flash_for)
    }

    /// When the next reminder chime is due, if one is.
    fn repeat_due_at(&self) -> Option<Instant> {
        // `?` on an Option: if there's no alert, return `None` right here.
        let alert = self.alert.as_ref()?;
        let reminder = self.reminder()?;
        if !alert.sound || self.config.muted || alert.repeats_done >= reminder.max_times {
            return None;
        }
        let every = Duration::from_secs(u64::from(reminder.every_secs));
        Some(alert.started + every * (alert.repeats_done + 1))
    }

    fn apply_window_size(&self, ctx: &egui::Context) {
        let size = window_size(&self.config, self.show_settings);
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(size.into()));
        fit_tooltips(ctx, size);
    }

    fn set_settings_open(&mut self, ctx: &egui::Context, open: bool) {
        if open && !self.show_settings {
            // Pick up schedules saved into the folder since we last looked.
            self.rescan_schedules();
        }
        self.show_settings = open;
        self.apply_window_size(ctx);
    }

    fn set_click_through(&mut self, ctx: &egui::Context, on: bool) {
        self.click_through = on;
        ctx.send_viewport_cmd(ViewportCommand::MousePassthrough(on));
        if on {
            // The settings panel can't be clicked any more, so close it.
            self.set_settings_open(ctx, false);
        }
    }

    fn set_visible(&mut self, ctx: &egui::Context, visible: bool) {
        self.visible = visible;
        ctx.send_viewport_cmd(ViewportCommand::Visible(visible));
    }

    fn handle_tray_action(&mut self, ctx: &egui::Context, action: TrayAction, now: Instant) {
        match action {
            TrayAction::TogglePause => {
                self.timer.toggle(now);
                self.alert = None;
            }
            TrayAction::ResetInterval => {
                self.timer.restart_interval(now);
                self.alert = None;
            }
            TrayAction::RestartSchedule => {
                self.timer.restart_schedule(now);
                self.alert = None;
            }
            TrayAction::SelectSchedule(name) => {
                self.config.schedule = name;
                self.reload_schedule(now);
                if let Some(tray) = &self.tray {
                    tray.fix_schedule_checks();
                }
            }
            TrayAction::ToggleVisible => self.set_visible(ctx, !self.visible),
            TrayAction::ToggleMute => self.config.muted = !self.config.muted,
            TrayAction::ToggleClickThrough => self.set_click_through(ctx, !self.click_through),
            TrayAction::ResetPosition => {
                ctx.send_viewport_cmd(ViewportCommand::OuterPosition(egui::pos2(40.0, 40.0)));
                self.set_visible(ctx, true);
            }
            TrayAction::InstallUpdate => self.install_update(ctx),
            TrayAction::Quit => ctx.send_viewport_cmd(ViewportCommand::Close),
        }
    }

    /// Normal view: a progress ring with the step's label, the time, and a status
    /// line inside it. While the pointer is over the widget, the status line gives
    /// way to Start/Pause and Skip icons, and a gear appears in the corner.
    fn main_view(&mut self, ui: &mut egui::Ui, rect: egui::Rect, now: Instant) {
        let accent = self.accent();
        let content = rect.shrink(10.0);

        // The ring fills the (square) content area. It's drawn directly with the
        // painter (nothing clickable), so drags on it move the window.
        let center = content.center();
        let painter = ui.painter();
        let fraction_left = 1.0 - self.timer.progress(now);
        let radius = content.width() / 2.0 - RING_WIDTH / 2.0;
        draw_ring(painter, center, radius, fraction_left, accent);
        painter.text(
            center - vec2(0.0, 1.0),
            Align2::CENTER_CENTER,
            format_mm_ss(self.timer.remaining(now)),
            digits_font(24.0),
            Color32::WHITE,
        );

        // Text above and below the time must fit inside the ring, which is
        // narrower there than across the middle.
        let row_offset = 26.0;
        let max_width = 84.0; // the ring's inner edge is about 85 wide at those rows

        // Above the time: the step's label.
        let flashing = self.is_flashing(now);
        let label = self.timer.step().label.to_uppercase();
        // During a "Whole widget" flash the background is already the accent
        // color, so use white.
        let whole_widget = self.config.flash_style == FlashStyle::WholeWidget;
        let (title, color) = match (flashing, whole_widget) {
            (true, true) => (format!("{label}!"), Color32::WHITE),
            (true, false) => (format!("{label}!"), accent),
            (false, _) => (label, accent),
        };
        paint_fitted(
            painter,
            center - vec2(0.0, row_offset),
            title,
            FontId::proportional(14.0),
            color,
            max_width,
        );

        // Below the time: Start/Pause and Skip while the pointer is over the
        // widget, otherwise a one-line status.
        let below = center + vec2(0.0, row_offset);
        if ui.rect_contains_pointer(rect) {
            // They've found the controls; the first-run hint can go.
            self.config.hover_hint_seen = true;
            let size = vec2(20.0, 20.0);
            let toggle = egui::Rect::from_center_size(below - vec2(12.0, 0.0), size);
            let skip = toggle.translate(vec2(24.0, 0.0));
            let toggle_icon = if self.timer.is_running() {
                Icon::Pause
            } else {
                Icon::Play
            };
            if icon_button(ui, toggle, toggle_icon).clicked() {
                self.timer.toggle(now);
            }
            if icon_button(ui, skip, Icon::Skip)
                .on_hover_text("Go to the next interval now")
                .clicked()
            {
                self.timer.skip(now);
            }
            // The gear sits in the top-right corner, outside the ring.
            let gear = egui::Rect::from_center_size(
                egui::pos2(rect.right() - 14.0, rect.top() + 14.0),
                vec2(16.0, 16.0),
            );
            if icon_button(ui, gear, Icon::Gear)
                .on_hover_text("Settings")
                .clicked()
            {
                self.set_settings_open(ui.ctx(), true);
            }
        } else {
            // `(String, Color32, f32)`: what to say, in what color, at what size.
            let (status, color, size) = if flashing {
                ("Switch!".to_owned(), Color32::WHITE, 11.0)
            } else if self.awaiting_click {
                ("Click to start".to_owned(), Color32::WHITE, 11.0)
            } else if self.click_through {
                ("Click-through".to_owned(), Color32::GRAY, 11.0)
            } else if self.schedule_error.is_some() {
                (
                    "Schedule error".to_owned(),
                    ui.visuals().warn_fg_color,
                    11.0,
                )
            } else if !self.config.hover_hint_seen {
                // Until the first hover, nothing on screen says the controls exist.
                // Slightly smaller, so the whole phrase fits inside the ring.
                ("Hover for controls".to_owned(), Color32::WHITE, 10.0)
            } else if !self.timer.is_running() {
                ("Paused".to_owned(), Color32::GRAY, 11.0)
            } else if let Some(rounds) = self.timer.step().rounds_text() {
                (rounds, Color32::GRAY, 11.0)
            } else {
                (String::new(), Color32::GRAY, 11.0)
            };
            paint_fitted(
                ui.painter(),
                below,
                status,
                FontId::proportional(size),
                color,
                max_width,
            );
        }
    }

    /// Compact mode: just the time, plus a thin progress bar along the bottom.
    /// While the pointer is over the widget, the time moves left to make room for
    /// Start/Pause and Skip icons.
    fn compact_view(&mut self, ui: &egui::Ui, rect: egui::Rect, now: Instant) {
        // egui repaints when the pointer moves or leaves the window, so this
        // updates by itself; no `wake_in` deadline needed.
        let hovered = ui.rect_contains_pointer(rect);
        let time = format_mm_ss(self.timer.remaining(now));
        if hovered {
            self.config.hover_hint_seen = true;
            let size = vec2(20.0, 20.0);
            let y = rect.center().y - 1.0;
            let skip = egui::Rect::from_center_size(egui::pos2(rect.right() - 20.0, y), size);
            let toggle = skip.translate(vec2(-22.0, 0.0));
            let toggle_icon = if self.timer.is_running() {
                Icon::Pause
            } else {
                Icon::Play
            };
            if icon_button(ui, toggle, toggle_icon).clicked() {
                self.timer.toggle(now);
            }
            if icon_button(ui, skip, Icon::Skip).clicked() {
                self.timer.skip(now);
            }
            // Centered in the space left of the icons.
            let text_center = egui::pos2((rect.left() + toggle.left()) / 2.0 + 2.0, y);
            ui.painter().text(
                text_center,
                Align2::CENTER_CENTER,
                time,
                digits_font(19.0),
                Color32::WHITE,
            );
        } else {
            ui.painter().text(
                rect.center() - vec2(0.0, 1.0),
                Align2::CENTER_CENTER,
                time,
                digits_font(24.0),
                Color32::WHITE,
            );
        }
        let painter = ui.painter();
        let track = egui::Rect::from_min_max(
            egui::pos2(rect.left() + CORNER_RADIUS, rect.bottom() - 5.0),
            egui::pos2(rect.right() - CORNER_RADIUS, rect.bottom() - 3.0),
        );
        let mut done = track;
        done.set_width(track.width() * self.timer.progress(now));
        painter.rect_filled(track, 1.0, Color32::from_white_alpha(25));
        painter.rect_filled(done, 1.0, self.accent());
    }

    fn settings_view(&mut self, ui: &mut egui::Ui, now: Instant) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Settings").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("×")
                    .on_hover_text("Close settings")
                    .clicked()
                {
                    self.set_settings_open(ui.ctx(), false);
                }
            });
        });
        ui.separator();

        // Everything below scrolls if it doesn't fit the window.
        egui::ScrollArea::vertical().show(ui, |ui| self.settings_contents(ui, now));
    }

    fn settings_contents(&mut self, ui: &mut egui::Ui, now: Instant) {
        section_heading(ui, "SCHEDULE", false);
        self.schedule_settings(ui, now);

        section_heading(ui, "ALERTS", true);
        // Settings the loaded schedule file sets for itself are shown, not editable.
        let from_file = self.timer.schedule().settings().clone();
        if let Some(style) = from_file.alerts {
            set_by_schedule(ui, style.label());
        } else {
            egui::ComboBox::from_id_salt("alert_style")
                .selected_text(self.config.alert_style.label())
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for style in AlertStyle::ALL_STYLES {
                        ui.selectable_value(&mut self.config.alert_style, style, style.label());
                    }
                })
                .response
                .on_hover_text("How you're told an interval has ended");
        }
        // Both flash rows are greyed out when the alert style doesn't flash at all.
        let flashes = self.alert_style().visual();
        ui.add_enabled_ui(flashes, |ui| {
            ui.horizontal(|ui| {
                ui.label("Flash style");
                egui::ComboBox::from_id_salt("flash_style")
                    .selected_text(self.config.flash_style.label())
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for style in FlashStyle::ALL_STYLES {
                            ui.selectable_value(&mut self.config.flash_style, style, style.label());
                        }
                    })
                    .response
                    .on_hover_text(
                        "Whole widget: the background pulses.\n\
                         Border: only the colored border pulses.\n\
                         Comet: a bright streak runs around the edge.\n\
                         Try them with Test alert.",
                    );
            });
        });
        if let Some(flash) = from_file.flash {
            set_by_schedule(ui, &format!("Flash for {}", describe_duration(flash)));
        } else {
            ui.add_enabled_ui(flashes, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Flash for");
                    ui.add(
                        egui::DragValue::new(&mut self.config.flash_secs)
                            .range(1..=600)
                            .suffix(" s"),
                    )
                    .on_hover_text(
                        "How long the widget flashes when an interval starts.\n\
                         Click it to stop sooner. Never longer than the interval.",
                    );
                });
            });
        }
        ui.add_space(4.0);
        ui.spacing_mut().slider_width = 85.0;
        ui.add(egui::Slider::new(&mut self.config.volume, 0.0..=1.0).text("Volume"));
        ui.checkbox(&mut self.config.muted, "Mute sound");
        match from_file.reminder {
            Some(Some(r)) => set_by_schedule(
                ui,
                &format!(
                    "Repeat chime every {} s, max {}×",
                    r.every_secs, r.max_times
                ),
            ),
            Some(None) => set_by_schedule(ui, "No repeat chime"),
            None => {
                ui.checkbox(
                    &mut self.config.repeat_reminder,
                    "Repeat chime until clicked",
                );
                // `add_enabled_ui` greys out (and disables) everything inside when false.
                ui.add_enabled_ui(self.config.repeat_reminder, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(24.0); // line up under the checkbox text
                        ui.label("every");
                        ui.add(
                            egui::DragValue::new(&mut self.config.repeat_every_secs)
                                .range(10..=600)
                                .suffix(" s"),
                        );
                        ui.label("max");
                        ui.add(
                            egui::DragValue::new(&mut self.config.repeat_max_times)
                                .range(1..=20)
                                .suffix("×"),
                        );
                    });
                });
            }
        }

        section_heading(ui, "APPEARANCE", true);
        // Names above the color buttons, so it's clear which is which.
        egui::Grid::new("colors")
            .num_columns(3)
            .min_col_width(56.0)
            .spacing(vec2(8.0, 2.0))
            .show(ui, |ui| {
                ui.label("Sit");
                ui.label("Stand");
                ui.label("Break").on_hover_text(
                    "Any schedule label other than sit or stand.\n\
                         A schedule file can give labels their own colors.",
                );
                ui.end_row();
                ui.color_edit_button_srgb(&mut self.config.sit_color);
                ui.color_edit_button_srgb(&mut self.config.stand_color);
                ui.color_edit_button_srgb(&mut self.config.break_color);
                ui.end_row();
            });
        ui.add_space(4.0);
        ui.add(egui::Slider::new(&mut self.config.opacity, 0.0..=1.0).text("Opacity"));
        ui.checkbox(&mut self.config.show_border, "Show border");
        ui.checkbox(&mut self.config.compact, "Compact (double-click)");

        // Click-through needs a way back: the hotkey, the tray menu, or both.
        let mut click_through = self.click_through;
        let way_back = self.hotkey.is_some() || self.tray.is_some();
        let hint = if way_back {
            format!(
                "Clicks pass through the widget.\nTurn off with {} or the tray menu.",
                hotkey::LABEL
            )
        } else {
            "Unavailable: no hotkey or tray icon to turn it off again.".to_owned()
        };
        if ui
            .add_enabled(
                way_back,
                egui::Checkbox::new(&mut click_through, "Click-through"),
            )
            .on_hover_text(&hint)
            .on_disabled_hover_text(&hint)
            .changed()
        {
            self.set_click_through(ui.ctx(), click_through);
        }

        section_heading(ui, "UPDATES", true);
        self.update_settings(ui);

        section_heading(ui, "", true);
        if ui
            .checkbox(&mut self.config.start_with_windows, "Start with Windows")
            .on_hover_text("Start the timer when you sign in to Windows")
            .changed()
        {
            self.apply_start_with_windows();
        }
        if let Some(err) = &self.startup_error {
            ui.label(
                RichText::new(format!("Couldn't change startup: {err}"))
                    .small()
                    .color(ui.visuals().warn_fg_color),
            );
        }
        ui.horizontal(|ui| {
            if ui.button("Test alert").clicked() {
                // As if the current interval had just ended.
                let next = self.timer.upcoming(1).clone();
                let following = self.timer.upcoming(2).clone();
                self.fire_alert(&next, &following, now);
            }
            if ui.button("Quit").clicked() {
                ui.ctx().send_viewport_cmd(ViewportCommand::Close);
            }
        });
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!(
                "Right-click: settings. Double-click: compact.\n{}: click-through on/off.",
                hotkey::LABEL
            ))
            .small()
            .weak(),
        );
    }

    /// The UPDATES section of the settings: the version status, and a button to
    /// check now or install what was found.
    fn update_settings(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.config.check_for_updates, "Check daily")
            .on_hover_text(
                "Look for a new version on GitHub once a day.\nInstalling always asks first.",
            );
        let status = ui.label(RichText::new(self.updates.status_text()).small().weak());
        if let Some(err) = self.updates.check_error() {
            status.on_hover_text(err);
        }
        // `if let ... else`: one button or the other, depending on whether an update was found.
        if let Some(version) = self.updates.available_version() {
            let label = format!("Update to {version}");
            let installing = self.updates.is_installing();
            if ui
                .add_enabled(!installing, egui::Button::new(label))
                .on_hover_text("Download and install it. The app restarts afterwards.")
                .clicked()
            {
                self.install_update(ui.ctx());
            }
        } else if ui
            .add_enabled(self.updates.can_check(), egui::Button::new("Check now"))
            .clicked()
        {
            self.updates.check_now(ui.ctx());
        }
    }

    /// The SCHEDULE section of the settings.
    fn schedule_settings(&mut self, ui: &mut egui::Ui, now: Instant) {
        let mut reload = false;
        let mut rescan = false;

        let selected = if self.config.schedule.is_empty() {
            "Plain sit/stand".to_owned()
        } else {
            schedule::display_name(&self.config.schedule)
        };
        egui::ComboBox::from_id_salt("schedule")
            .selected_text(selected)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                reload |= ui
                    .selectable_value(&mut self.config.schedule, String::new(), "Plain sit/stand")
                    .changed();
                // `if let Ok(...)`: list nothing if the folder couldn't be read.
                if let Ok(names) = &self.schedule_files {
                    for name in names {
                        reload |= ui
                            .selectable_value(
                                &mut self.config.schedule,
                                name.clone(),
                                schedule::display_name(name),
                            )
                            .changed();
                    }
                }
            })
            .response
            .on_hover_text(
                "Plain sit/stand, or a .sstc file from the schedules folder, e.g.\n\n\
                 3x\n- 30m sit\n- 15m stand\n\n- 15m break\n\n\
                 `- <length> <label>` is an interval, `3x` repeats the lines\n\
                 below it up to a blank line (or the indented lines below it).",
            );
        ui.add_space(2.0);

        // Plain sit/stand: its two lengths. A schedule file: what's in it, or what's wrong.
        let warn = ui.visuals().warn_fg_color;
        if let Some(err) = &self.schedule_error {
            ui.label(
                RichText::new(format!("{err}\nUsing plain sit/stand for now."))
                    .small()
                    .color(warn),
            );
        } else if self.config.schedule.is_empty() {
            let mut durations_changed = false;
            egui::Grid::new("durations").num_columns(2).show(ui, |ui| {
                ui.label("Sit (minutes)");
                // `|=` keeps `durations_changed` true once any widget reports a change.
                durations_changed |= ui
                    .add(egui::DragValue::new(&mut self.config.sit_minutes).range(1..=240))
                    .changed();
                ui.end_row();
                ui.label("Stand (minutes)");
                durations_changed |= ui
                    .add(egui::DragValue::new(&mut self.config.stand_minutes).range(1..=240))
                    .changed();
                ui.end_row();
            });
            if durations_changed {
                // Same two steps, so this keeps the current mode and the time counted so far.
                self.timer.set_schedule(self.config.simple_schedule());
            }
        } else {
            let schedule = self.timer.schedule();
            ui.label(
                RichText::new(format!(
                    "{} intervals, {} per cycle",
                    schedule.step_count(),
                    describe_duration(schedule.cycle_length())
                ))
                .small()
                .weak(),
            );
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui
                .button("Open folder")
                .on_hover_text("Show the schedules folder, to add or edit .sstc files")
                .clicked()
                && let Some(folder) = self.config.schedules_folder_path()
            {
                // The default folder may have been deleted; make it again so there's
                // something to open. (A folder typed by the user is left as it is.)
                if self.config.schedules_folder.trim().is_empty() {
                    let _ = std::fs::create_dir_all(&folder);
                }
                crate::platform::open_folder(&folder);
            }
            if ui
                .button("Reload")
                .on_hover_text("Re-read the schedule after editing it")
                .clicked()
            {
                reload = true;
                rescan = true;
            }
        });
        let mut folder_changed = false;
        // Rarely changed, so tucked away until clicked.
        egui::CollapsingHeader::new("Other folder").show(ui, |ui| {
            let default_folder = Config::default_schedules_folder()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            // Typing edits the draft; the setting itself changes when the box loses
            // focus (click elsewhere, or press Enter).
            let folder_box = ui.add(
                egui::TextEdit::singleline(&mut self.folder_draft)
                    .hint_text("Default folder")
                    .desired_width(ui.available_width()),
            );
            // `on_hover_text` takes `folder_box` by value, so check `lost_focus` first.
            folder_changed =
                folder_box.lost_focus() && self.folder_draft != self.config.schedules_folder;
            folder_box.on_hover_text(format!(
                "Folder of .sstc schedules. Leave empty for\n{default_folder}"
            ));
            ui.horizontal(|ui| {
                // Greyed out while the dialog is already open.
                let browse =
                    ui.add_enabled(self.folder_pick.is_none(), egui::Button::new("Browse…"));
                if browse
                    .on_hover_text("Choose the folder in a window")
                    .clicked()
                {
                    self.browse_for_folder(ui.ctx());
                }
                if ui
                    .add_enabled(
                        !self.config.schedules_folder.is_empty(),
                        egui::Button::new("Default"),
                    )
                    .on_hover_text("Go back to the default folder")
                    .clicked()
                {
                    self.set_schedules_folder(String::new(), now);
                }
            });
        });
        if folder_changed {
            self.set_schedules_folder(self.folder_draft.clone(), now);
        }
        if let Err(err) = &self.schedule_files {
            ui.label(RichText::new(err).small().color(warn));
        }
        if rescan {
            self.rescan_schedules();
        }
        if reload {
            self.reload_schedule(now);
        }

        ui.add_space(6.0);
        match self.timer.schedule().settings().auto_start {
            Some(true) => set_by_schedule(ui, "Next interval starts by itself"),
            Some(false) => set_by_schedule(ui, "Next interval waits for a click"),
            None => {
                if ui
                    .checkbox(
                        &mut self.config.auto_start_next,
                        "Start next interval by itself",
                    )
                    .on_hover_text("Off: when an interval ends, wait for a click on the widget")
                    .changed()
                {
                    self.timer.set_auto_continue(self.config.auto_start_next);
                }
            }
        }
    }
}

// Implementing the `eframe::App` trait is how we plug into eframe: it calls
// `logic` and then `ui` every time the window needs to be redrawn. While the
// window is hidden, only `logic` runs, so everything that must keep working
// (timer, alerts, tray, saving) lives there.
impl eframe::App for SitStandApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();

        // Someone launched the app again (Start menu, taskbar pin): show the widget.
        if self.show_requests.try_iter().count() > 0 {
            self.set_visible(ctx, true);
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }

        // The folder dialog closed: use the folder picked, if any.
        if let Some(receiver) = &self.folder_pick {
            match receiver.try_recv() {
                Ok(folder) => {
                    self.folder_pick = None;
                    self.set_schedules_folder(folder.display().to_string(), now);
                }
                // Cancelled: the dialog's thread ended without sending anything.
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.folder_pick = None,
                Err(std::sync::mpsc::TryRecvError::Empty) => {} // still open
            }
        }

        if now.duration_since(self.last_schedule_scan) >= SCHEDULE_RESCAN {
            self.rescan_schedules();
        }

        // Tray menu clicks. `map(...).unwrap_or_default()` gives an empty list if
        // there's no tray.
        let actions = self.tray.as_ref().map(|t| t.actions()).unwrap_or_default();
        for action in actions {
            self.handle_tray_action(ctx, action, now);
        }

        // `.as_ref()` turns `&Option<Hotkey>` into `Option<&Hotkey>`, so we can look
        // inside without moving the hotkey out of `self`.
        if self.hotkey.as_ref().is_some_and(|h| h.was_pressed()) {
            self.set_click_through(ctx, !self.click_through);
        }

        if let Some(TimerEvent::IntervalEnded) = self.timer.tick(now) {
            let next = self.timer.step().clone();
            let following = self.timer.next_step().clone();
            self.fire_alert(&next, &following, now);
            // If the new interval didn't start by itself (the app setting, the
            // schedule's `start next`, or its own `[wait]`), remember that we're
            // waiting, so a click on the widget starts it.
            self.awaiting_click = !self.timer.is_running();
        }
        // However the timer got started (widget click, button, or tray), stop waiting.
        if self.timer.is_running() {
            self.awaiting_click = false;
        }

        // Reminder chimes for an alert nobody has acknowledged.
        if self.repeat_due_at().is_some_and(|due| now >= due) {
            // `if let Some(a) = &mut self.alert` borrows the alert mutably so we can change it.
            if let Some(a) = &mut self.alert {
                a.repeats_done += 1;
                let mode = a.mode;
                self.audio.play_chime(mode, self.config.volume);
            }
        }
        // Forget the alert once it has nothing left to do.
        if self.alert.is_some() && !self.is_flashing(now) && self.repeat_due_at().is_none() {
            self.alert = None;
        }

        // Remember where the window is. (Coordinates far off-screen mean the window
        // is minimized; don't save those.)
        let outer = ctx.input(|i| i.viewport().outer_rect);
        if let Some(rect) = outer
            && rect.min.x > -10_000.0
            && rect.min.y > -10_000.0
        {
            self.config.window_pos = Some([rect.min.x, rect.min.y]);
        }

        // Save changed settings, at most once per SAVE_DELAY.
        let dirty = self.config != self.saved_config;
        if dirty && now.duration_since(self.last_save) >= SAVE_DELAY {
            self.config.save();
            self.saved_config = self.config.clone();
            self.last_save = now;
        }

        // Finished update checks, and the next one if it's due.
        let next_update_check = self.updates.poll(ctx, now, self.config.check_for_updates);

        // Keep the tray icon, tooltip and menu in sync.
        let minutes_left = self.timer.remaining(now).as_secs().div_ceil(60);
        let paused = if self.timer.is_running() {
            ""
        } else {
            " (paused)"
        };
        let tray_state = TrayState {
            color: self.step_rgb(self.timer.step()),
            tooltip: format!(
                "{}: {minutes_left} min left{paused}",
                self.timer.step().label.to_uppercase()
            ),
            running: self.timer.is_running(),
            visible: self.visible,
            muted: self.config.muted,
            click_through: self.click_through,
            schedules: self.schedule_files.clone().unwrap_or_default(),
            selected_schedule: self.config.schedule.clone(),
            // `.map(str::to_owned)` turns the borrowed `Option<&str>` into an owned
            // `Option<String>`, since `TrayState` keeps its own copy.
            update_version: self.updates.available_version().map(str::to_owned),
        };
        if let Some(tray) = &mut self.tray {
            tray.update(tray_state);
        }

        // egui only redraws when something happens, so ask for the next wake-up:
        // whichever comes first of the reasons below.
        let mut wake_in: Vec<Duration> = Vec::new();
        if self.is_flashing(now) {
            wake_in.push(Duration::from_millis(33));
        }
        if self.timer.is_running() {
            let until_next_second = self.timer.remaining(now).subsec_millis() as u64;
            wake_in.push(Duration::from_millis(until_next_second + 10));
        }
        if let Some(due) = self.repeat_due_at() {
            wake_in.push(due.saturating_duration_since(now));
        }
        if self.config != self.saved_config {
            wake_in.push(SAVE_DELAY.saturating_sub(now.duration_since(self.last_save)));
        }
        if self.tray.is_some() {
            wake_in
                .push(SCHEDULE_RESCAN.saturating_sub(now.duration_since(self.last_schedule_scan)));
        }
        if let Some(due) = next_update_check {
            wake_in.push(due);
        }
        if let Some(&soonest) = wake_in.iter().min() {
            ctx.request_repaint_after(soonest);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        let rect = ui.max_rect();

        // Any click on the widget acknowledges an alert.
        if self.alert.is_some() && ui.input(|i| i.pointer.primary_pressed()) {
            self.alert = None;
        }

        // An invisible "button" covering the whole window. It is added first, so the
        // real buttons added later sit on top of it and get their clicks normally.
        let background = ui.interact(
            rect,
            egui::Id::unique("background"),
            egui::Sense::click_and_drag(),
        );
        if background.drag_started() {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if background.secondary_clicked() {
            self.set_settings_open(ui.ctx(), !self.show_settings);
        }
        if background.double_clicked() && !self.show_settings {
            // If the double-click's first click just started a waiting interval (the
            // `else if` below, a moment ago), that wasn't meant as "start": undo it.
            // The window allows for the press itself as well as the gap between clicks.
            let window = ui.ctx().options(|o| {
                o.input_options.max_double_click_delay + o.input_options.max_click_duration
            });
            if let Some(started) = self.click_started_at.take()
                && now.duration_since(started).as_secs_f64() <= window
            {
                self.timer.pause(now);
                // It was a fresh interval, so going back to zero loses nothing.
                self.timer.restart_interval(now);
                self.awaiting_click = true;
            }
            self.config.compact = !self.config.compact;
            self.apply_window_size(ui.ctx());
        } else if self.awaiting_click && background.clicked() && !self.show_settings {
            // `else`: the second click of a double-click also counts as a click, and
            // must not start the interval we just put back.
            self.timer.start(now);
            self.click_started_at = Some(now);
        }

        // During an alert's flash: seconds since it started, and its color.
        let flash = match &self.alert {
            Some(a) if self.is_flashing(now) => {
                Some((now.duration_since(a.started).as_secs_f32(), rgb(a.color)))
            }
            _ => None,
        };
        let flash_style = self.config.flash_style;

        // Background panel. A "Whole widget" flash pulses it toward the accent color
        // at full opacity.
        let (fill, alpha) = match flash {
            Some((t, color)) if flash_style == FlashStyle::WholeWidget => {
                (mix(BG_COLOR, color, 0.15 + 0.6 * pulse(t)), 1.0)
            }
            // Settings stay readable even with a see-through widget.
            _ if self.show_settings => (BG_COLOR, self.config.opacity.max(0.97)),
            _ => (BG_COLOR, self.config.opacity),
        };
        let fill =
            Color32::from_rgba_unmultiplied(fill.r(), fill.g(), fill.b(), (alpha * 255.0) as u8);
        let painter = ui.painter();
        painter.rect_filled(rect, CORNER_RADIUS, fill);

        // Border. A "Border" flash pulses it thicker and brighter. During a "Comet"
        // the streak running along the edge replaces it entirely.
        match flash {
            Some((t, color)) if flash_style == FlashStyle::Comet => {
                draw_comet(painter, rect, color, t);
            }
            Some((t, color)) if flash_style == FlashStyle::Border => {
                let stroke = Stroke::new(
                    1.5 + 2.5 * pulse(t),
                    mix(color, Color32::WHITE, 0.35 * pulse(t)),
                );
                painter.rect_stroke(rect, CORNER_RADIUS, stroke, StrokeKind::Inside);
            }
            _ if self.config.show_border => {
                let stroke = Stroke::new(1.5, self.accent());
                painter.rect_stroke(rect, CORNER_RADIUS, stroke, StrokeKind::Inside);
            }
            _ => {} // border turned off in the settings
        }

        if self.show_settings {
            egui::Frame::new()
                .inner_margin(10.0)
                .show(ui, |ui| self.settings_view(ui, now));
        } else if self.config.compact {
            self.compact_view(ui, rect, now);
        } else {
            self.main_view(ui, rect, now);
        }
    }

    /// Called once when the app closes: make sure the latest settings are on disk.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.config.save();
    }

    /// The color behind everything we draw. Fully transparent, so only our
    /// rounded panel is visible.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_points_are_evenly_spaced_all_the_way_around() {
        let rect = egui::Rect::from_min_size(Pos2::ZERO, vec2(220.0, 130.0));
        let (radius, step) = (12.0, 2.0);
        let points = rounded_rect_outline(rect, radius, step);
        // Perimeter: the straight parts of the four sides, plus a full circle of corners.
        let perimeter = 2.0 * (220.0 + 130.0) - 8.0 * radius + TAU * radius;
        let expected = (perimeter / step) as usize;
        assert!(
            points.len().abs_diff(expected) <= 2,
            "{} points",
            points.len()
        );
        // Neighbours are `step` apart (a little less where the path bends at a corner).
        for pair in points.windows(2) {
            let gap = pair[0].distance(pair[1]);
            assert!(gap > step * 0.9 && gap <= step + 0.01, "gap {gap}");
        }
        // Every point lies on the rectangle's edge region, inside its bounds.
        assert!(points.iter().all(|p| rect.expand(0.01).contains(*p)));
    }
}
