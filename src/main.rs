// In release builds, don't open a console window next to the app (Windows only;
// ignored elsewhere). Debug builds keep the console so `eprintln!` output is visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Each `mod` line pulls in the file of the same name (src/app.rs, ...).
mod alert;
mod app;
mod config;
mod hotkey;
mod platform;
mod schedule;
mod single_instance;
mod timer;
mod tray;

use eframe::egui;

fn main() -> eframe::Result {
    // If the app is already running, it has just been asked to show itself; we're done.
    let Some(instance) = single_instance::SingleInstance::acquire() else {
        return Ok(());
    };

    platform::register_app_identity();
    // No settings file yet: this is the first run, so set up the example schedule.
    // `is_some_and` is true only if there is a path *and* the closure says yes.
    let first_run = config::Config::path().is_some_and(|path| !path.exists());
    let config = config::Config::load();
    if first_run {
        config::Config::create_default_schedules_folder();
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Sit-Stand Timer")
        .with_app_id("sitstand") // lets Linux desktops match the window to its icon
        .with_inner_size(app::window_size(&config, false))
        .with_resizable(false)
        .with_decorations(false) // no title bar or border
        .with_transparent(true) // lets our rounded, translucent panel show through
        .with_always_on_top();
    // Window / taskbar icon. The PNG is built into the program like the font.
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/logo.png")) {
        viewport = viewport.with_icon(icon);
    }
    if let Some(position) = config.window_pos {
        viewport = viewport.with_position(position);
    }

    let options = eframe::NativeOptions {
        viewport,
        // `..Default::default()` = "every other field gets its default value".
        ..Default::default()
    };

    eframe::run_native(
        "Sit-Stand Timer",
        options,
        // A closure that eframe calls once the window exists, to build our app.
        // `move` hands ownership of `config` and `instance` to the closure.
        Box::new(move |cc| Ok(Box::new(app::SitStandApp::new(cc, config, instance)))),
    )
}
