# Sit/Stand Timer

A small, always-on-top desktop widget that times sitting and standing intervals
for a sit/stand desk. Native Rust ([egui](https://github.com/emilk/egui)), no
browser engine, one small `.exe`.

## Install Rust (Windows)

1. Install the **Visual Studio Build Tools** with the **"Desktop development with C++"**
   workload (Rust needs Microsoft's linker):
   ```
   winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
   ```
2. Install Rust via rustup:
   ```
   winget install Rustlang.Rustup
   ```
3. Open a **new** terminal and check: `rustc --version` and `cargo --version`.

If a build fails with linker errors (`link.exe not found`), step 1 is missing.

## Build and run

```
cargo run                 # debug build, with a console for log messages
cargo build --release     # optimized build: target\release\sitstand.exe
cargo test                # unit tests (timer logic, config parsing)
cargo clippy              # lints
```

The release `.exe` is self-contained (font, sounds and logo are built in). Close
any running copy before rebuilding, or Windows will report "Access is denied"
because the file is in use.

## Installer (Start menu entry, uninstall in Settings → Apps)

One-time setup: `cargo install cargo-packager --locked`

Then:

```
cargo packager --release
```

This builds the release `.exe` and wraps it in
`target\release\sitstand_<version>_x64-setup.exe`. Running that installs for
your user only (no admin prompt) into `%LOCALAPPDATA%\Sit-Stand Timer`, adds a
**Sit-Stand Timer** Start menu entry, and registers an uninstaller under
Settings → Apps. To update, quit the app (tray → Quit) and run a newer setup
file over the old install. Bump `version` in `Cargo.toml` so Settings shows the
new number.

Installer options live under `[package.metadata.packager]` in `Cargo.toml`.

Only one copy of the app runs at a time: launching it again (Start menu,
taskbar pin) just brings the running widget forward, even if it was hidden.

The logo is `assets/logo.png` (window icon) and `assets/logo.ico` (`.exe`
icon, embedded by `build.rs`). To use your own, replace both files: the `.ico`
should contain several sizes (16-256 px).

## Using it

| Action | How |
| --- | --- |
| Move the widget | Drag it anywhere |
| Start / pause, next interval | Hover the widget: Play/Pause and Skip icons appear (inside the ring, or beside the time in compact mode) |
| Open / close settings | Right-click the widget, or the gear icon (top-right corner, on hover) |
| Compact mode (time only) | Double-click the widget, or the setting |
| Dismiss an alert | Click the widget |
| Click-through on/off | **Ctrl+Alt+Shift+S** (works from any app), or the tray menu |
| Everything else | Right-click the tray icon (colored dot, bottom-right of the taskbar; it may be under the **^** overflow arrow) |

The tray menu has Start/Pause, Restart interval, Restart schedule, a Schedule picker, Show/Hide widget, Mute alerts,
Click-through, Reset widget position (if the widget ends up off-screen, e.g.
after unplugging a monitor) and Quit. Hover the tray icon to see the time left.

## Settings

Change them in the settings panel; they're saved automatically to:

```
%APPDATA%\SitStand\config\config.toml
```

You can also edit that file by hand while the app is closed. Missing entries get
their default values and out-of-range values are clamped. If the file can't be
parsed, it's renamed to `config.toml.bad` and the app starts with defaults.

| Key | Default | Meaning |
| --- | --- | --- |
| `sit_minutes`, `stand_minutes` | 30, 15 | Interval lengths (1-240) |
| `sit_color`, `stand_color`, `break_color` | `[70, 140, 255]`, `[50, 205, 120]`, `[240, 170, 60]` | Accent colors, `[red, green, blue]` |
| `schedules_folder` | `""` | Folder of `.sstc` schedules (see below); empty = `schedules` next to `config.toml` |
| `schedule` | `""` | File name of the schedule to follow, from that folder; empty = plain sit/stand |
| `opacity` | 0.85 | Background opacity, 0.0-1.0 (text stays solid) |
| `show_border` | true | Draw the colored border (Border and Comet alerts still show on the edge) |
| `alert_style` | `"all"` | `"all"`, `"visual_only"`, `"sound_only"` or `"silent"` |
| `volume` / `muted` | 0.7 / false | Chime volume (0.0-1.0) and mute |
| `auto_start_next` | true | `false` = wait for a click before starting the next interval |
| `repeat_reminder` | false | Repeat the chime until the alert is clicked... |
| `repeat_every_secs`, `repeat_max_times` | 60, 3 | ...this often, at most this many times |
| `flash_secs` | 60 | How long the widget flashes when an interval starts (1-600; never longer than the interval) |
| `flash_style` | `"whole_widget"` | How the widget flashes: `"whole_widget"` (background pulses), `"border"` (border pulses) or `"comet"` (a streak runs around the edge) |
| `compact` | false | Show only the time |
| `window_pos` | (unset) | Remembered window position |

Click-through is deliberately *not* saved, so the widget always starts clickable.

To change the built-in defaults, edit `impl Default for Config` in `src/config.rs`.

## Custom schedules (`.sstc` files)

Instead of a plain sit/stand cycle, the timer can follow a schedule you write,
pomodoro style. Keep as many as you like as `.sstc` files in one folder and pick
one from the **Schedule** dropdown in the settings, or from tray → **Schedule**.

The folder is `%APPDATA%\SitStand\config\schedules` unless you type another path
in the box under the dropdown. The default folder is created on the first run with a
`Pomodoro.sstc` in it (the pomodoro technique). **Open folder** shows it in
Explorer. After editing a file, press **Reload** to re-read it. (New files show up
in the lists by themselves within a few seconds.)

```
# Pomodoro: start each focus session with a click; breaks start by themselves.
*****
start next: auto
long break: #b07cff
*****

3x
- 25m sit [wait]
- 5m stand

- 25m sit [wait]
- 20m long break
```

`- <length> <label>` is an interval, `3x` repeats the lines below it (up to a
blank line, or everything indented under it), `[...]` options change how one
interval starts, and the `*****` block sets colors and settings for the whole
schedule.

**The full format reference is [docs/sstc-format.md](docs/sstc-format.md)**:
every rule, option and setting, the limits, and more examples.

## Project layout

```
src/main.rs      window setup, loads config, starts the app
src/app.rs       the widget: drawing, clicks, alerts, saving
src/timer.rs     pure timer logic (no UI), unit-tested
src/schedule.rs  schedules and the .sstc file format, unit-tested
src/config.rs    settings, TOML load/save
src/alert.rs     chime playback and desktop notifications
src/tray.rs      tray icon and menu
src/hotkey.rs    the global click-through shortcut
src/platform.rs  Windows-only fixes for the borderless window's edges
assets/          bundled font (JetBrains Mono, OFL license) and chime sounds
docs/            sstc-format.md: the schedule file format reference
```

## Notes

- Windows is the tested platform. The code is written to build elsewhere, but a
  Linux build needs GTK development packages for the tray icon, and on Wayland
  windows generally can't force themselves always-on-top.
