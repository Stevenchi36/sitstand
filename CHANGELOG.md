# Changelog

What changed in each release. The section for a version becomes its GitHub
Release notes, so add one (moving things out of **Unreleased**) before tagging.

## [Unreleased]

### Added

- **Start with Windows** setting (bottom of the settings): starts the timer when
  you sign in. Uninstalling removes the startup entry.
- A one-time "Hover for controls" hint under the time, until the pointer first
  goes over the widget.
- A changelog; each release's notes now come from it.

### Changed

- Schedule files: a label's kind (sit, stand or break, which picks the chime and
  default color) now comes from its whole first word. `sit down`, `standing` and
  `stand-up` work as before, but labels like `situps` or `standup meeting` are now
  breaks instead of sit and stand.
- Schedule files have a written compatibility promise: the format only grows, so
  files that work now keep working (`docs/sstc-format.md`, "Compatibility").

### Fixed

- Long tooltips (such as an update error) were cut off at the edge of the window.
  They now wrap to its width.

## [0.2.0] - 2026-11-08

### Added

- In-app updates. The app checks GitHub once a day; when a newer version is out,
  **Update to x.y.z** appears in the tray menu and in Settings → UPDATES. It
  downloads the installer, checks its signature, installs it and restarts.
- Settings → UPDATES: **Check daily** (on by default) and **Check now**.

## [0.1.2] - 2026-11-08

First public release.

- Always-on-top widget timing sit, stand and break intervals, with a progress
  ring; Play/Pause, Skip and Settings appear on hover.
- Compact mode (double-click): just the time, with the same hover controls.
- Custom schedules in `.sstc` files (see `docs/sstc-format.md`), picked from
  settings or the tray. A Pomodoro example is created on first run.
- Alerts when an interval ends: chime, flashing widget (three styles), desktop
  notification, optional repeat chime.
- Tray menu, a global click-through shortcut (Ctrl+Alt+Shift+S), and an
  installer that needs no admin rights.
