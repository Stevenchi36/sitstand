//! Small OS-specific fixes that egui/eframe don't offer settings for.

/// Windows 11 draws extra edges around borderless windows that don't line up
/// with the rounded panel we paint ourselves:
/// - its own thin gray border and rounded corners, and
/// - a 1px line along the top, a side effect of the drop shadow winit adds to
///   borderless windows.
///
/// Turn all of that off, so our own drawing is the only outline.
#[cfg(target_os = "windows")]
pub fn remove_os_window_frame(cc: &eframe::CreationContext<'_>) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
        DwmSetWindowAttribute,
    };
    // A trait must be imported (`use`d) before its methods can be called;
    // this one adds Windows-only methods such as `set_undecorated_shadow` to winit's Window.
    use winit::platform::windows::WindowExtWindows;

    if let Some(window) = cc.winit_window() {
        window.set_undecorated_shadow(false);
    }

    // `let Ok(...) = ... else { return }`: if getting the handle fails, quietly do nothing.
    let Ok(handle) = cc.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        return;
    };
    let hwnd = win32.hwnd.get() as windows_sys::Win32::Foundation::HWND;

    let no_rounding = DWMWCP_DONOTROUND;
    let no_border = DWMWA_COLOR_NONE;
    // `unsafe` is required for calling into the Windows API: Rust can't check what
    // C code does with the pointers we pass. Here we pass pointers to two local
    // variables plus their sizes, which is exactly what the API expects. On older
    // Windows versions that don't know these settings, the calls just fail harmlessly.
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &no_rounding as *const _ as *const core::ffi::c_void,
            size_of_val(&no_rounding) as u32,
        );
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR as u32,
            &no_border as *const _ as *const core::ffi::c_void,
            size_of_val(&no_border) as u32,
        );
    }
}

/// On other platforms there's nothing to fix, so this does nothing.
#[cfg(not(target_os = "windows"))]
pub fn remove_os_window_frame(_cc: &eframe::CreationContext<'_>) {}

/// The app's identity on Windows (an "AppUserModelID"). Windows uses it to
/// name notifications and to group the running window with a taskbar pin.
/// Must match `identifier` under `[package.metadata.packager]` in Cargo.toml,
/// because the installer stamps the same ID onto the Start menu shortcut.
#[cfg(target_os = "windows")]
pub const APP_ID: &str = "com.sitstand.timer";

/// Tell Windows who we are. Call once at startup, before any notification.
/// - Registers the app ID with a display name and icon (per-user registry,
///   no admin needed), so notifications say "Sit-Stand Timer" instead of
///   "Windows PowerShell", even when not installed (e.g. `cargo run`).
/// - Marks this process with the app ID, so its taskbar button groups with a pin.
#[cfg(target_os = "windows")]
pub fn register_app_identity() {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegSetKeyValueW};
    use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

    /// Rust strings → the 0-terminated UTF-16 that Windows API functions expect.
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    // Notifications show an icon from a real image file, so keep a copy of
    // the logo next to the settings file.
    let icon_path = crate::config::Config::path()
        .map(|p| p.with_file_name("icon.png"))
        .filter(|p| {
            p.parent()
                .is_some_and(|dir| std::fs::create_dir_all(dir).is_ok())
                && std::fs::write(p, include_bytes!("../assets/logo.png")).is_ok()
        });

    let key = wide(&format!(r"Software\Classes\AppUserModelId\{APP_ID}"));
    let mut values = vec![("DisplayName", "Sit-Stand Timer".to_owned())];
    if let Some(path) = &icon_path {
        values.push(("IconUri", path.display().to_string()));
    }
    for (name, value) in values {
        let name = wide(name);
        let data = wide(&value);
        // `unsafe`: Windows API call. The pointers are to the 0-terminated buffers
        // above, and the size is their length in bytes (2 bytes per UTF-16 unit).
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                REG_SZ,
                data.as_ptr() as *const core::ffi::c_void,
                (data.len() * 2) as u32,
            );
        }
    }

    let id = wide(APP_ID);
    unsafe {
        SetCurrentProcessExplicitAppUserModelID(id.as_ptr());
    }
}

/// Other platforms identify apps differently (e.g. `with_app_id` in main.rs on Linux).
#[cfg(not(target_os = "windows"))]
pub fn register_app_identity() {}

/// Adds or removes this program from the ones Windows starts when you sign in.
/// Uses the per-user `Run` registry key (no admin needed); the entry also shows
/// up, and can be switched off, in Task Manager → Startup apps.
#[cfg(target_os = "windows")]
pub fn set_start_with_windows(on: bool) -> Result<(), String> {
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RegDeleteKeyValueW, RegSetKeyValueW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Run");
    let name = wide("Sit-Stand Timer");

    let status = if on {
        // `?` returns early with the error if the path can't be found;
        // `map_err` first turns that error into a message.
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        // Quoted, because the install folder has spaces in it.
        let command = wide(&format!("\"{}\"", exe.display()));
        // `unsafe`: Windows API call. The pointers are to the 0-terminated buffers
        // above, and the size is the command's length in bytes.
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                REG_SZ,
                command.as_ptr() as *const core::ffi::c_void,
                (command.len() * 2) as u32,
            )
        }
    } else {
        // Same buffers as above; deleting a value that isn't there is fine.
        match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) } {
            ERROR_FILE_NOT_FOUND => ERROR_SUCCESS,
            status => status,
        }
    };
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!("Windows error {status}"))
    }
}

/// Other platforms: not supported, so there's nothing to change.
#[cfg(not(target_os = "windows"))]
pub fn set_start_with_windows(_on: bool) -> Result<(), String> {
    Ok(())
}

/// Show a folder in the system file manager (Explorer on Windows).
pub fn open_folder(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    let program = "explorer";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let program = "xdg-open";
    // `spawn` starts it without waiting for it to finish.
    if let Err(err) = std::process::Command::new(program).arg(path).spawn() {
        eprintln!("Could not open {}: {err}", path.display());
    }
}
