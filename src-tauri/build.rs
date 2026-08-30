fn main() {
    println!("cargo:rerun-if-changed=migrations");
    // Tauri embeds icon.png in development builds. Track it explicitly so
    // regenerated app icons are picked up without a manual Cargo clean.
    println!("cargo:rerun-if-changed=icons/icon.png");
    tauri_build::build()
}
