use serde::Serialize;
use tauri::{AppHandle, Runtime};

/// Static application/runtime information.
/// Mirrored by `AppInfo` in `src/services/types.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub identifier: String,
    pub os: &'static str,
    pub arch: &'static str,
    pub tauri_version: &'static str,
    pub webview_version: Option<String>,
    pub debug: bool,
}

pub fn collect<R: Runtime>(app: &AppHandle<R>) -> AppInfo {
    let package = app.package_info();
    AppInfo {
        name: package.name.clone(),
        version: package.version.to_string(),
        identifier: app.config().identifier.clone(),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        tauri_version: tauri::VERSION,
        webview_version: tauri::webview_version().ok(),
        debug: cfg!(debug_assertions),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_camel_case_keys_matching_typescript() {
        let info = AppInfo {
            name: "Islandy".into(),
            version: "0.1.0".into(),
            identifier: "com.islandy.desktop".into(),
            os: "windows",
            arch: "x86_64",
            tauri_version: "2.12.5",
            webview_version: None,
            debug: true,
        };
        let json = serde_json::to_value(info).unwrap();
        let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "arch",
                "debug",
                "identifier",
                "name",
                "os",
                "tauriVersion",
                "version",
                "webviewVersion"
            ]
        );
        assert!(json["webviewVersion"].is_null());
    }
}
