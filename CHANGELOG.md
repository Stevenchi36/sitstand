# Changelog

What changed in each release. The section for a version becomes its GitHub
Release notes, so add one (moving things out of **Unreleased**) before tagging.

## [Unreleased]

### Added

- A one-time "Hover for controls" hint under the time, until the pointer first
  goes over the widget.
- A changelog; each release's notes now come from it.

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
