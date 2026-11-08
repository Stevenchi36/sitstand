# Plan: in-app updates

**Status:** idea, not started (written 2026-11-08, at version 0.1.2).

Today, updating means quitting the app, downloading the new
`sitstand_<version>_x64-setup.exe` and running it over the old install. That's
fine for one machine but not for a released app. This page records the plan for
letting the app update itself.

## Recommended approach

Use **`cargo-packager-updater`** (the companion crate to `cargo-packager`, which
already builds the installer), with releases hosted on **GitHub Releases**.

### What the user sees

1. About once a day, the app checks a small `latest.json` on GitHub in the
   background. It lists the newest version, the installer's download URL and
   its signature.
2. If there's a newer version, the tray menu and the settings panel show
   **"Update to x.y.z"**. Updates never install without asking, so a routine
   isn't interrupted mid-interval.
3. Clicking it downloads the installer, verifies its signature, runs it with no
   windows to click through, and restarts the app.

It's still the NSIS installer underneath, just run silently. That's normal on
Windows, and because the app installs per-user there's no admin (UAC) prompt.
Settings and schedules live in `%APPDATA%\SitStand`, so they carry over.

### Signing

`cargo packager signer generate` makes a key pair:

- The **public key** is built into the app.
- The **private key** signs each release. Keep it outside the repo (and as a
  secret in CI). If it's lost, existing installs can't verify future updates.

The app refuses any download that isn't signed with that key, even if the
download URL or `latest.json` is tampered with.

### Releasing, once this is in place

1. Bump `version` in `Cargo.toml`.
2. `cargo packager --release` with the signing key set (via environment variables).
3. Upload the setup `.exe` and the updated `latest.json` to a GitHub Release.

A GitHub Actions workflow can do steps 2 and 3 when a version tag (e.g. `v0.1.3`)
is pushed.

### Fitting it into the app

- The check and download run on a background thread and report back over an
  `mpsc` channel plus `ctx.request_repaint()`, like the tray, hotkey and folder
  dialog (see "Background events" in `CLAUDE.md`).
- The daily check needs its own deadline in `logic`'s `wake_in` list, or it
  won't happen while the widget sits idle.
- Possibly a setting to turn update checks off (`Config` field + README table).

## Alternatives considered

| Option | Verdict |
| --- | --- |
| **winget** | Worth adding later so people can find and install the app (`winget install ...`). Updating is still something users have to run themselves, so it doesn't replace in-app updates. |
| **Microsoft Store / MSIX** | Automatic updates for free, but needs certificates, Store review and a different install model. Too much for a small widget. |
| **`self_update` crate** | Swaps the `.exe` directly instead of running the installer, so the version in Settings → Apps would fall out of date. |

## Before starting

- [ ] Create a GitHub repo for the app. Public is easiest: a private repo's
      release downloads need a login. Its URL becomes the update address
      baked into the app.
- [ ] Check `cargo-packager-updater`'s current version and API. The details
      above haven't been verified against its docs yet.
- [ ] Generate the signing keys and store the private key somewhere safe.
