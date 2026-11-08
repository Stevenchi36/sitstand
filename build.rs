// A build script: cargo compiles and runs this *before* compiling the app.
// Here it builds the logo and a description into the Windows .exe, which is
// what File Explorer, the Start menu, taskbar pins and Task Manager show.

fn main() {
    println!("cargo:rerun-if-changed=assets/logo.ico");

    // Only when building for Windows (this script also runs for other targets).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/logo.ico");
        resource.set("ProductName", "Sit-Stand Timer");
        resource.set("FileDescription", "Sit-Stand Timer");
        if let Err(err) = resource.compile() {
            // Not fatal: the app still works, just with the default icon.
            println!("cargo:warning=Could not embed the Windows icon: {err}");
        }
    }
}
