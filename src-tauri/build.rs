fn main() {
    // Declaring the app's commands in an app manifest makes Tauri generate
    // `allow-*`/`deny-*` permissions for them. Commands are then *denied*
    // unless a capability grants them (see capabilities/island.json).
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_app_info",
            "connect_island",
            "get_island_layouts",
            "set_island_frame",
            "set_island_interactive",
            "open_system_settings",
            "connect_media",
            "get_media_snapshot",
            "select_media_session",
            "media_control",
            "media_seek",
            "get_media_artwork",
        ]),
    ))
    .expect("failed to run tauri-build");
}
