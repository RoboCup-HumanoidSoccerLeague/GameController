//! This build script embeds the icon into the executable on Windows. The other platforms take
//! the icon from their distribution (and all of them set the window icon at runtime).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=icons/icon.ico");
    #[cfg(windows)]
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        winresource::WindowsResource::new()
            .set_icon("icons/icon.ico")
            .compile()
            .expect("could not embed the icon into the executable");
    }
}
