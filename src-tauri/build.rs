fn main() {
    // Declaring the app's commands in an app manifest makes Tauri generate
    // `allow-*`/`deny-*` permissions for them. Commands are then *denied*
    // unless a capability grants them (see capabilities/island.json).
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["get_app_info"])),
    )
    .expect("failed to run tauri-build");
}
