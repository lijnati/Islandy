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
            "connect_system",
            "get_system_snapshot",
            "set_system_focus",
            "set_master_volume",
            "set_audio_muted",
            "set_radio_state",
            "set_display_brightness",
            "connect_activity",
            "get_activity_snapshot",
            "dismiss_activity",
            "clear_activity_history",
            "set_quiet_mode",
            "set_activity_source",
            "get_claude_integration",
            "install_claude_integration",
            "remove_claude_integration",
        ]),
    ))
    .expect("failed to run tauri-build");
}
