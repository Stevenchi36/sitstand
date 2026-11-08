# Plan: in-app updates

**Status:** implemented for 0.2.0 (2026-11-08): `src/update.rs`, the UPDATES
section in settings, the tray item, and signing plus `latest.json` in the release
workflow. Not yet tested end to end; that needs two signed releases (see
"Testing" below).

Today, updating means quitting the app, downloading the new
`sitstand_<version>_x64-setup.exe` from GitHub Releases and running it over the
old install. This page records the plan for letting the app update itself.

## Approach

Use **`cargo-packager-updater`** (0.2.3, the companion crate to `cargo-packager`,
which already builds the installer). Releases are already published to
**GitHub Releases** by `.github/workflows/release.yml`.

### What the user sees

1. About once a day, the app checks a small `latest.json` on GitHub in the
   background. It lists the newest version, the installer's download URL and
   its signature.
2. If there's a newer version, the tray menu and the settings panel show
   **"Update to x.y.z"**. Updates never install without asking, so a routine
   isn't interrupted mid-interval.
3. Clicking it downloads the installer and verifies its signature. The app then
   closes, the installer runs showing only a progress bar, and it starts the
   app again.

Because the app installs per-user there's no admin (UAC) prompt. Settings and
schedules live in `%APPDATA%\SitStand`, so they carry over.

### What the crate does (checked against its 0.2.3 source)

- **Blocking API.** `check_update(current_version, Config)` returns
  `Result<Option<Update>>` (`None` = already up to date). Then
  `update.download_and_install()`, or `download_extended(on_chunk, on_done)`
  followed by `install(bytes)` to show download progress. It makes blocking
  HTTP requests (reqwest, rustls), so both steps run on a background thread.
- **`Config`** = `endpoints: Vec<Url>`, `pubkey: String`, and optional
  `windows: WindowsConfig` (install mode; the default `Passive` runs the NSIS
  installer with `/P /R`: progress bar only, then relaunch).
- **The endpoint can be a static file.** It just needs to answer `200` with JSON
  `{ version, url, signature, format: "nsis", notes?, pub_date? }`. The crate
  compares versions itself, so a fixed URL works:
  `https://github.com/Stevenchi36/sitstand/releases/latest/download/latest.json`
- **`install()` ends the process** with `std::process::exit(0)` right after
  starting the installer. eframe's `on_exit` never runs, so **save the config
  before calling it**, or the last second of setting changes is lost.
- The installer cargo-packager generates already supports `/P` (passive) and
  `/R` (run the app afterwards).

### Signing

`cargo packager signer generate --path <somewhere outside the repo>` makes a key
pair (optionally with `--password`):

- The **public key** is built into the app (a `const` in the updater module).
- The **private key** signs each release. Store it as GitHub Actions secrets
  `CARGO_PACKAGER_SIGN_PRIVATE_KEY` (+ `CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD`),
  with a backup copy kept somewhere safe. If it's lost, installed copies can't
  verify any future update and users have to reinstall by hand.

With those environment variables set, `cargo packager --release` also writes
`sitstand_<version>_x64-setup.exe.sig` next to the installer.

The app refuses any download that isn't signed with that key, even if the
download URL or `latest.json` is tampered with.

## Work to do

### 1. Release workflow (`.github/workflows/release.yml`)

- Pass the two signing secrets to the `cargo packager --release` step.
- After packaging, write `latest.json`: the version, the download URL
  `https://github.com/Stevenchi36/sitstand/releases/download/v<version>/sitstand_<version>_x64-setup.exe`,
  the contents of the `.sig` file, `"format": "nsis"`, and `pub_date`.
- Attach `latest.json` to the release alongside the installer.
- Fail the run if the `.sig` file is missing (so an unsigned release can't
  break updates).

### 2. App (`src/update.rs`, wired up in `app.rs`)

- Current version from `env!("CARGO_PKG_VERSION")`.
- Check on startup and then every 24 hours on a background thread. Report back
  over an `mpsc` channel plus `ctx.request_repaint()`, like the tray, hotkey and
  folder dialog (see "Background events" in `CLAUDE.md`).
- The daily check needs its own deadline in `logic`'s `wake_in` list, or it
  won't happen while the widget sits idle.
- When an update is found: an **"Update to x.y.z"** tray item and a row in
  settings. Clicking either saves the config, then downloads and installs on
  the background thread. Show "Downloading…" meanwhile, and the error text if
  it fails.
- A `check_for_updates: bool` setting (default `true`) on `Config`: add it to
  `impl Default` and the README settings table.
- Network failures are quiet: no update shown, try again next time.

### 3. Docs

- README: the new setting, and how updating works for users.
- README "Releasing" section and `CLAUDE.md`: signing secrets and `latest.json`.

## Things to know

- **The first updater-enabled version must be installed by hand.** 0.1.2 has no
  updater, so anyone on it downloads the next release (say 0.2.0) once; from
  then on updates are in-app.
- **Testing:** install that first updater build, then publish a second release
  (e.g. 0.2.1) and check the update offer, download, install and relaunch end to
  end. Watch for the single-instance check: the relaunched copy must not find
  the old one still running.
- **Binary size:** the HTTP and TLS libraries took the installer from 3.8 MB to
  4.6 MB.

## Alternatives considered

| Option | Verdict |
| --- | --- |
| **winget** | Worth adding later so people can find and install the app (`winget install ...`). Updating is still something users have to run themselves, so it doesn't replace in-app updates. |
| **Microsoft Store / MSIX** | Automatic updates for free, but needs certificates, Store review and a different install model. Too much for a small widget. |
| **`self_update` crate** | Swaps the `.exe` directly instead of running the installer, so the version in Settings → Apps would fall out of date. |

## Before starting

- [x] Public GitHub repo: <https://github.com/Stevenchi36/sitstand>
- [x] Tag-triggered release workflow publishing the installer
- [x] Check `cargo-packager-updater`'s current version and API (0.2.3, see above)
- [x] Generate the signing keys, add the private key as GitHub secrets, and back
      it up somewhere outside the repo
- [x] Steps 1-3 above
- [ ] Release 0.2.0, install it by hand, then release 0.2.1 and update in-app
