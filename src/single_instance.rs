//! Makes sure only one copy of the app runs. Launching it a second time just
//! asks the running copy to show its widget, then exits.
//!
//! On Windows this uses a named "event": a tiny signal object that any process
//! can find by name. The first copy creates it and waits on it; a later copy
//! finds it already exists, triggers it, and quits.

use std::sync::mpsc::{self, Receiver};

use eframe::egui;

/// Held by the first (and only) running copy of the app.
pub struct SingleInstance {
    /// The Windows event's handle, stored as a plain number so it can be moved
    /// to another thread (raw pointers can't be, by default).
    #[cfg(target_os = "windows")]
    event: isize,
}

impl SingleInstance {
    /// Returns `None` if another copy is already running (it has been asked to
    /// show itself, so this copy should just exit).
    #[cfg(target_os = "windows")]
    pub fn acquire() -> Option<SingleInstance> {
        use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError};
        use windows_sys::Win32::System::Threading::{CreateEventW, SetEvent};

        // Windows API strings are UTF-16 and must end with a 0.
        let name: Vec<u16> = "Local\\SitStandTimer.ShowWidget\0".encode_utf16().collect();
        // `unsafe`: calling the Windows API. We pass a valid, 0-terminated name.
        let event = unsafe { CreateEventW(std::ptr::null(), 0, 0, name.as_ptr()) };
        if event.is_null() {
            // Couldn't create it at all: rather than refuse to start, just run.
            return Some(SingleInstance { event: 0 });
        }
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe {
                SetEvent(event);
                CloseHandle(event);
            }
            return None;
        }
        Some(SingleInstance {
            event: event as isize,
        })
    }

    /// Starts a background thread that waits for "show yourself" requests from
    /// later launches. Each request arrives as a `()` on the returned channel.
    #[cfg(target_os = "windows")]
    pub fn listen(self, ctx: &egui::Context) -> Receiver<()> {
        use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
        use windows_sys::Win32::System::Threading::{INFINITE, WaitForSingleObject};

        let (sender, requests) = mpsc::channel();
        if self.event == 0 {
            return requests;
        }
        let ctx = ctx.clone();
        // `move` gives the thread ownership of `self`, `sender` and `ctx`.
        std::thread::spawn(move || {
            let event = self.event as windows_sys::Win32::Foundation::HANDLE;
            // Sleep until the event is triggered, forward it, and wait again.
            while unsafe { WaitForSingleObject(event, INFINITE) } == WAIT_OBJECT_0 {
                if sender.send(()).is_err() {
                    break; // the app has shut down
                }
                ctx.request_repaint();
            }
        });
        requests
    }

    /// Other platforms: no single-instance check (yet), every launch runs.
    #[cfg(not(target_os = "windows"))]
    pub fn acquire() -> Option<SingleInstance> {
        Some(SingleInstance {})
    }

    #[cfg(not(target_os = "windows"))]
    pub fn listen(self, _ctx: &egui::Context) -> Receiver<()> {
        // A channel nobody ever sends on.
        mpsc::channel().1
    }
}
