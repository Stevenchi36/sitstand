# Sit-Stand Timer

Always-on-top desktop widget (Rust, egui/eframe) that times sit/stand/break intervals.
Windows is the target platform. User-facing docs, build commands and the settings
table live in `README.md`.

## Code style

The owner is learning Rust. Comments explain the *Rust* as well as the intent:
`?`, `if let`, `matches!`, ownership moves, `#[derive]`, closures capturing, etc.
Write new code the same way: short comments that teach the idiom the first time it
appears in a file, plain-English doc comments on every item.

## Architecture

- **Pure logic, unit-tested:** `timer.rs` (countdown state machine) and `schedule.rs`
  (the step list and the `.sstc` parser). They hold no UI code, and every method that
  needs the time takes `now: Instant`, so tests fake time with `t0 + 5 * MIN`. Put new
  behaviour here when it can be tested without a window.
- **The timer reports, the app reacts:** `Timer::tick` returns a `TimerEvent`; `app.rs`
  decides on sound/flash/notification (`fire_alert`). Keep the timer unaware of alerts.
- **Plain sit/stand is a schedule too:** `Schedule::simple` builds a two-step schedule,
  so the timer has one code path. A `Step`'s `Mode` (Sit/Stand/Break) comes from its
  label and picks the chime and default color; a schedule file's `*****` block can
  give a label its own color (`Step::color`, resolved in `SitStandApp::step_rgb`).
- **Schedule files can override app settings:** the `*****` block's
  `ScheduleSettings` (alerts, start next, repeat chime, flash) and each step's
  `[...]` `StepOptions` win over `Config` while that schedule is loaded. Resolve a
  setting through the app helpers (`alert_style()`, `reminder()`,
  `flash_duration()`, `AlertPlan::new`, `Timer::tick` for auto-start) rather than
  reading `Config` directly. Only settings that describe the routine belong in the
  file; display and audio-device settings (volume, mute, opacity, compact,
  click-through, window position) stay app-wide.
- **`.sstc` format:** specified in the module doc at the top of `schedule.rs`
  (the source of truth). The complete user reference is `docs/sstc-format.md`;
  any format change updates both, plus the `POMODORO` example's header comments.
- **Background events** (tray menu, global hotkey, second-launch "show yourself") all
  use one pattern: a callback on another thread sends into an `mpsc` channel and calls
  `ctx.request_repaint()`; `logic` drains the channel.

## egui gotchas

- `logic` runs every frame, even while the window is hidden; `ui` runs only when it's
  visible. Timer ticks, alerts, tray sync, saving and rescans belong in `logic`.
- egui redraws only when asked. `logic` builds a `wake_in` list of "wake me in X";
  any new periodic work must push its own deadline there, or it stalls while idle.
- Windows are fixed sizes (`MAIN_SIZE`, `COMPACT_SIZE`, `SETTINGS_SIZE` in `app.rs`).
  The settings panel has about 200px of usable width: a wide grid column or row
  widens everything under it and gets clipped. Check new settings widgets fit.
- `ui.with_layout(right_to_left/left_to_right, ...)` inside a vertical layout claims
  the whole remaining height. For a single row, wrap it in `ui.horizontal(...)`.

## Config

`config.rs`: one `Config` struct saved as TOML in `%APPDATA%\SitStand\config\`.
The app saves whenever `config != saved_config` (debounced), so any field on `Config`
persists; transient state belongs on `SitStandApp` (click-through is deliberately
unsaved). When adding a field: give it a value in `impl Default`, clamp it in
`sanitized()` if it has a range, and add it to the README settings table.
`#[serde(default)]` keeps old config files loading.

## Building and running

- Only one copy runs at a time (`single_instance.rs`). If any copy is running,
  including the installed one in `%LOCALAPPDATA%\Sit-Stand Timer\`, `cargo run`
  just brings that copy forward and exits silently. Quit it first (tray → Quit).
- A running `target\debug\sitstand.exe` is locked, so the build fails with
  "Access is denied" until it's stopped.
- Before finishing a change: `cargo fmt`, `cargo clippy --all-targets` (keep it at
  zero warnings), `cargo test`.
- Releasing: bump `version` in `Cargo.toml`, then `cargo packager --release`.

## Checking the UI

There are no UI tests. To see a layout change, run the debug build and screenshot
its window from PowerShell: `GetWindowRect` on the process's `MainWindowHandle`
(after `SetProcessDPIAware`), then `Graphics.CopyFromScreen`. Simulated input via
`SetCursorPos` + `mouse_event` reaches the widget: right-click opens/closes settings,
the wheel scrolls it. Restore the cursor afterwards; it's the owner's desktop.
