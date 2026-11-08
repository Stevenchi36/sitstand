//! A system-wide shortcut (Ctrl+Alt+Shift+S) that toggles click-through mode.
//! While click-through is on the widget ignores the mouse, so this is the way back.

use std::sync::mpsc::{self, Receiver};

use eframe::egui;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

pub const LABEL: &str = "Ctrl+Alt+Shift+S";

pub struct ClickThroughHotkey {
    /// Must stay alive: dropping the manager unregisters the shortcut.
    _manager: GlobalHotKeyManager,
    /// The receiving end of a channel. The hotkey callback (running elsewhere)
    /// sends `()` into it on every key press; the app checks it each frame.
    presses: Receiver<()>,
}

impl ClickThroughHotkey {
    /// Registers the shortcut. Returns `None` if that fails (for example if
    /// another app already owns the same shortcut).
    pub fn register(ctx: &egui::Context) -> Option<Self> {
        let manager = match GlobalHotKeyManager::new() {
            Ok(manager) => manager,
            Err(err) => {
                eprintln!("Global hotkeys unavailable: {err}");
                return None;
            }
        };
        let hotkey = HotKey::new(
            Some(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT),
            Code::KeyS,
        );
        if let Err(err) = manager.register(hotkey) {
            eprintln!("Could not register {LABEL}: {err}");
            return None;
        }

        // A channel is a one-way pipe between pieces of code that may run on
        // different threads: `sender` goes into the callback, `presses` stays here.
        let (sender, presses) = mpsc::channel();
        // egui only redraws when asked, so the callback also wakes up the app.
        // `ctx.clone()` gives the closure its own handle (cheap: egui's Context is shared inside).
        let ctx = ctx.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.id == hotkey.id() && event.state == HotKeyState::Pressed {
                // `let _ =` : we deliberately ignore the result (it only fails after
                // the app has shut down).
                let _ = sender.send(());
                ctx.request_repaint();
            }
        }));

        Some(ClickThroughHotkey {
            _manager: manager,
            presses,
        })
    }

    /// True if the shortcut was pressed since the last call.
    pub fn was_pressed(&self) -> bool {
        // Drain everything waiting in the channel; count it as one press if there was any.
        self.presses.try_iter().count() > 0
    }
}
