//! System tray icon and its right-click menu.

use std::sync::mpsc::{self, Receiver};

use eframe::egui;
use tray_icon::menu::{
    CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu,
};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// The schedule items change as files come and go, so instead of a fixed list of
/// ids, each one's id is this prefix plus its file name ("" for plain sit/stand).
const SCHEDULE_ID_PREFIX: &str = "schedule:";

/// What the user picked from the tray menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayAction {
    TogglePause,
    ResetInterval,
    RestartSchedule,
    /// A schedule file name from the Schedule submenu, or "" for plain sit/stand.
    SelectSchedule(String),
    ToggleVisible,
    ToggleMute,
    ToggleClickThrough,
    ResetPosition,
    Quit,
}

/// Everything the tray displays. Kept so we only talk to the OS when something changed.
#[derive(Debug, Clone, PartialEq)]
pub struct TrayState {
    pub color: [u8; 3],
    pub tooltip: String,
    pub running: bool,
    pub visible: bool,
    pub muted: bool,
    pub click_through: bool,
    /// The `.sstc` files to offer, and the one in use ("" = plain sit/stand).
    pub schedules: Vec<String>,
    pub selected_schedule: String,
}

pub struct Tray {
    icon: TrayIcon,
    schedule_menu: Submenu,
    plain_item: CheckMenuItem,
    /// One per schedule file, in the same order as `TrayState::schedules`.
    schedule_items: Vec<CheckMenuItem>,
    pause_item: MenuItem,
    show_item: MenuItem,
    mute_item: CheckMenuItem,
    click_through_item: CheckMenuItem,
    actions: Receiver<TrayAction>,
    /// What's currently shown. `None` until the first `update`.
    shown: Option<TrayState>,
}

impl Tray {
    /// Creates the tray icon. Returns `None` if the OS refuses (the app still works without it).
    pub fn new(ctx: &egui::Context) -> Option<Tray> {
        let pause_item = MenuItem::new("Start", true, None);
        let reset_item = MenuItem::new("Restart interval", true, None);
        let restart_item = MenuItem::new("Restart schedule", true, None);
        let schedule_menu = Submenu::new("Schedule", true);
        let plain_item =
            CheckMenuItem::with_id(SCHEDULE_ID_PREFIX, "Plain sit/stand", true, false, None);
        let show_item = MenuItem::new("Hide widget", true, None);
        let mute_item = CheckMenuItem::new("Mute alerts", true, false, None);
        let click_through_item = CheckMenuItem::new("Click-through", true, false, None);
        let position_item = MenuItem::new("Reset widget position", true, None);
        let quit_item = MenuItem::new("Quit", true, None);

        let menu = Menu::new();
        // `and_then` runs the second step only if the first one worked.
        let appended = schedule_menu.append(&plain_item).and_then(|()| {
            menu.append_items(&[
                &pause_item,
                &reset_item,
                &restart_item,
                &schedule_menu,
                &PredefinedMenuItem::separator(),
                &show_item,
                &mute_item,
                &click_through_item,
                &position_item,
                &PredefinedMenuItem::separator(),
                &quit_item,
            ])
        });
        if let Err(err) = appended {
            eprintln!("Could not build tray menu: {err}");
            return None;
        }

        // Pair each menu item's id with the action it stands for. `.clone()` makes our
        // own copy of the id, because the items themselves stay owned by this struct.
        let id_to_action: Vec<(MenuId, TrayAction)> = vec![
            (pause_item.id().clone(), TrayAction::TogglePause),
            (reset_item.id().clone(), TrayAction::ResetInterval),
            (restart_item.id().clone(), TrayAction::RestartSchedule),
            (show_item.id().clone(), TrayAction::ToggleVisible),
            (mute_item.id().clone(), TrayAction::ToggleMute),
            (
                click_through_item.id().clone(),
                TrayAction::ToggleClickThrough,
            ),
            (position_item.id().clone(), TrayAction::ResetPosition),
            (quit_item.id().clone(), TrayAction::Quit),
        ];

        // Same pattern as the hotkey: forward menu clicks into a channel and wake the app.
        let (sender, actions) = mpsc::channel();
        let ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            // `iter().find(...)` returns the first matching pair, as an `Option`.
            let action = if let Some(name) = event.id.0.strip_prefix(SCHEDULE_ID_PREFIX) {
                Some(TrayAction::SelectSchedule(name.to_owned()))
            } else {
                id_to_action
                    .iter()
                    .find(|(id, _)| *id == event.id)
                    .map(|(_, action)| action.clone())
            };
            if let Some(action) = action {
                let _ = sender.send(action);
                ctx.request_repaint();
            }
        }));

        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Sit/Stand Timer")
            .with_icon(dot_icon([128, 128, 128])?)
            .build();
        let icon = match icon {
            Ok(icon) => icon,
            Err(err) => {
                eprintln!("Could not create tray icon: {err}");
                return None;
            }
        };

        Some(Tray {
            icon,
            schedule_menu,
            plain_item,
            schedule_items: Vec::new(),
            pause_item,
            show_item,
            mute_item,
            click_through_item,
            actions,
            shown: None,
        })
    }

    /// Menu choices made since the last call.
    pub fn actions(&self) -> Vec<TrayAction> {
        self.actions.try_iter().collect()
    }

    /// Bring the icon, tooltip and menu labels in line with the app's state.
    pub fn update(&mut self, state: TrayState) {
        if self.shown.as_ref() == Some(&state) {
            return; // nothing changed
        }
        // Only rebuild the icon image when the color changed.
        if self
            .shown
            .as_ref()
            .is_none_or(|old| old.color != state.color)
        {
            let _ = self.icon.set_icon(dot_icon(state.color));
        }
        let _ = self.icon.set_tooltip(Some(&state.tooltip));
        self.pause_item
            .set_text(if state.running { "Pause" } else { "Start" });
        self.show_item.set_text(if state.visible {
            "Hide widget"
        } else {
            "Show widget"
        });
        self.mute_item.set_checked(state.muted);
        self.click_through_item.set_checked(state.click_through);
        if self
            .shown
            .as_ref()
            .is_none_or(|old| old.schedules != state.schedules)
        {
            self.rebuild_schedule_items(&state.schedules);
        }
        self.check_schedule(&state.selected_schedule);
        self.shown = Some(state);
    }

    /// Clicking a check item flips its checkmark by itself, even when it's the schedule
    /// already in use (so nothing else changes and `update` skips it). Call this after a
    /// schedule click to put the checkmark back where it belongs.
    pub fn fix_schedule_checks(&self) {
        if let Some(state) = &self.shown {
            self.check_schedule(&state.selected_schedule);
        }
    }

    fn rebuild_schedule_items(&mut self, schedules: &[String]) {
        for item in self.schedule_items.drain(..) {
            let _ = self.schedule_menu.remove(&item);
        }
        for name in schedules {
            let id = format!("{SCHEDULE_ID_PREFIX}{name}");
            let label = crate::schedule::display_name(name);
            let item = CheckMenuItem::with_id(id, label, true, false, None);
            if self.schedule_menu.append(&item).is_ok() {
                self.schedule_items.push(item);
            }
        }
    }

    fn check_schedule(&self, selected: &str) {
        self.plain_item.set_checked(selected.is_empty());
        for item in &self.schedule_items {
            let name = item.id().0.strip_prefix(SCHEDULE_ID_PREFIX).unwrap_or("");
            item.set_checked(name == selected);
        }
    }
}

/// A 32x32 filled circle in the given color, with smooth (anti-aliased) edges.
fn dot_icon(color: [u8; 3]) -> Option<Icon> {
    const SIZE: u32 = 32;
    let center = (SIZE as f32 - 1.0) / 2.0;
    let radius = 13.0;
    // 4 bytes (red, green, blue, alpha) per pixel, row by row.
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let distance = ((x as f32 - center).powi(2) + (y as f32 - center).powi(2)).sqrt();
            // Fully opaque inside, fading out over one pixel at the edge.
            let alpha = (radius - distance + 0.5).clamp(0.0, 1.0);
            rgba.extend_from_slice(&[color[0], color[1], color[2], (alpha * 255.0) as u8]);
        }
    }
    // `.ok()` turns a `Result` into an `Option`, discarding the error.
    Icon::from_rgba(rgba, SIZE, SIZE).ok()
}
