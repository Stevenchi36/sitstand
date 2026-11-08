# Sit/Stand Timer

A small, always-on-top desktop widget that times sitting and standing intervals
for a sit/stand desk. Native Rust ([egui](https://github.com/emilk/egui)), no
browser engine, one small `.exe`.

## Download

Get `sitstand_<version>_x64-setup.exe` from the
[latest release](https://github.com/Stevenchi36/sitstand/releases/latest) and run
it. It installs for your user only (no admin prompt), and later versions update
from inside the app. What changed in each version: [CHANGELOG.md](CHANGELOG.md).

### Windows security warnings

The app isn't code-signed (that costs money), so Windows doesn't know who made it
and may warn about it. It's built from this repository's source by GitHub Actions;
see `.github/workflows/release.yml`.

- **"Windows protected your PC"** (SmartScreen) when running the installer: click
  **More info**, then **Run anyway**.
- **Microsoft Defender** may flag it as a trojan, typically
  `Trojan:Win32/Bearfoos.A!ml`. The `!ml` means a machine-learning guess, a common
  false alarm for new unsigned programs. To allow it: Windows Security → Virus &
  threat protection → Protection history → open the entry → **Actions** →
  **Allow on device**, then run the installer again.

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

Only one copy of the app runs at a time: launching it again (Start menu,
taskbar pin) just brings the running widget forward, even if it was hidden.

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
| `check_for_updates` | true | Look for a new release on GitHub once a day (installing still asks) |
| `hover_hint_seen` | false | Set once the pointer has been over the widget; hides the "Hover for controls" hint |

Click-through is deliberately *not* saved, so the widget always starts clickable.

To change the built-in defaults, edit `impl Default for Config` in `src/config.rs`.

## Custom schedules (`.sstc` files)

Instead of a plain sit/stand cycle, the timer can follow a schedule you write,
pomodoro style. Keep as many as you like as `.sstc` files in one folder and pick
one from the **Schedule** dropdown in the settings, or from tray → **Schedule**.

The folder is `%APPDATA%\SitStand\config\schedules` unless you choose another
under **Other folder** below the dropdown (**Browse…**, or type a path). The
default folder is created on the first run with a `Pomodoro.sstc` in it (the
pomodoro technique). **Open folder** shows it in Explorer. After editing a file,
press **Reload** to re-read it. (New files show up in the lists by themselves
within a few seconds.)

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

## Updates

From 0.2.0 on, the app checks GitHub once a day. When a newer release is out,
**Update to x.y.z** appears at the top of the tray menu and in Settings →
UPDATES. Clicking it downloads the installer, checks its signature, runs it with
only a progress bar, and restarts the app. Settings and schedules are kept.

**Privacy:** the app only goes online for that daily check, and to download an
update when you click **Update to x.y.z**. The check fetches the latest release's
`latest.json` from GitHub and sends nothing about you or your usage (GitHub sees
an ordinary download request). To turn it off, untick
Settings → UPDATES → **Check daily**; **Check now** still works when you want it.

Without in-app updates (or from 0.1.x), quit the app (tray → Quit) and run a
newer setup file over the old install.

---

## Development

### Install Rust (Windows)

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

### Build and run

```
cargo run                 # debug build, with a console for log messages
cargo build --release     # optimized build: target\release\sitstand.exe
cargo test                # unit tests (timer logic, config parsing)
cargo clippy              # lints
```

The release `.exe` is self-contained (font, sounds and logo are built in). Close
any running copy before rebuilding, or Windows will report "Access is denied"
because the file is in use.

### Installer (Start menu entry, uninstall in Settings → Apps)

One-time setup: `cargo install cargo-packager --locked`

Then:

```
cargo packager --release
```

This builds the release `.exe` and wraps it in
`target\release\sitstand_<version>_x64-setup.exe`. Running that installs for
your user only (no admin prompt) into `%LOCALAPPDATA%\Sit-Stand Timer`, adds a
**Sit-Stand Timer** Start menu entry, and registers an uninstaller under
Settings → Apps. Installer options live under `[package.metadata.packager]` in
`Cargo.toml`.

The logo is `assets/logo.png` (window icon) and `assets/logo.ico` (`.exe`
icon, embedded by `build.rs`). To use your own, replace both files: the `.ico`
should contain several sizes (16-256 px).

### Releasing

1. Move the **Unreleased** entries in `CHANGELOG.md` into a new
   `## [x.y.z] - <date>` section.
2. Bump `version` in `Cargo.toml` to match, and commit.
3. Push a matching tag:

   ```
   git tag v0.2.1
   git push origin main --tags
   ```

The Release workflow (`.github/workflows/release.yml`) checks the tag matches
`Cargo.toml` and that `CHANGELOG.md` has a section for it (which becomes the
release notes), runs the tests, builds and signs the installer, and publishes a
GitHub Release with the installer, its `.sig` and `latest.json` (what the in-app
updater reads).

Signing uses the repo secrets `CARGO_PACKAGER_SIGN_PRIVATE_KEY` and
`CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD`; the matching public key is
`PUBLIC_KEY` in `src/update.rs`. Keep a backup of the private key and its
password: without them, installed copies can't accept updates.

CI (`.github/workflows/ci.yml`) runs `cargo fmt --check`, clippy (no warnings
allowed) and the tests on every push and pull request.

### Project layout

```
src/main.rs      window setup, loads config, starts the app
src/app.rs       the widget: drawing, clicks, alerts, saving
src/timer.rs     pure timer logic (no UI), unit-tested
src/schedule.rs  schedules and the .sstc file format, unit-tested
src/config.rs    settings, TOML load/save
src/alert.rs     chime playback and desktop notifications
src/tray.rs      tray icon and menu
src/hotkey.rs    the global click-through shortcut
src/update.rs    checking GitHub for a new release and installing it
src/platform.rs  Windows-only fixes for the borderless window's edges
assets/          bundled font (JetBrains Mono, OFL license) and chime sounds
docs/            sstc-format.md (schedule file format), auto-update-plan.md
```

## Notes

- Windows is the tested platform. The code is written to build elsewhere, but a
  Linux build needs GTK development packages for the tray icon, and on Wayland
  windows generally can't force themselves always-on-top.

## License

MIT, see [LICENSE](LICENSE). The bundled JetBrains Mono font is under the SIL Open
Font License (`assets/OFL.txt`).
