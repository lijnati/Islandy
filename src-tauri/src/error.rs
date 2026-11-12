use serde::{ser::SerializeStruct, Serialize, Serializer};

/// Errors that can cross the IPC boundary.
///
/// Serialized as `{ kind, message }`; mirrored by `NativeErrorPayload`
/// in `src/services/types.ts`.
#[derive(Debug, thiserror::Error)]
pub enum IslandyError {
    #[error("window `{0}` was not found")]
    WindowNotFound(&'static str),
    #[error("no monitor is available to place the island on")]
    NoMonitor,
    #[error("invalid argument: {0}")]
    InvalidArgument(&'static str),
    #[cfg_attr(windows, allow(dead_code))] // only constructed off Windows
    #[error("{0}")]
    Unsupported(&'static str),
    #[cfg_attr(not(windows), allow(dead_code))] // only constructed by Win32 code
    #[error("platform integration failed: {0}")]
    Platform(String),
    #[error(transparent)]
    Media(#[from] crate::services::media::MediaError),
    #[error(transparent)]
    System(#[from] crate::services::system::SystemError),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
}

impl IslandyError {
    fn kind(&self) -> &'static str {
        match self {
            Self::WindowNotFound(_) => "window",
            Self::NoMonitor => "monitor",
            Self::InvalidArgument(_) => "invalid",
            Self::Unsupported(_) => "unsupported",
            Self::Platform(_) => "platform",
            Self::Media(e) => e.kind(),
            Self::System(e) => e.kind(),
            Self::Tauri(_) => "internal",
        }
    }
}

impl Serialize for IslandyError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("IslandyError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

pub type Result<T, E = IslandyError> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_the_frontend_error_shape() {
        let json = serde_json::to_value(IslandyError::NoMonitor).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "kind": "monitor",
                "message": "no monitor is available to place the island on"
            })
        );
        let json = serde_json::to_value(IslandyError::WindowNotFound("island")).unwrap();
        assert_eq!(json["kind"], "window");
        let json = serde_json::to_value(IslandyError::from(
            crate::services::media::MediaError::SessionNotFound,
        ))
        .unwrap();
        assert_eq!(json["kind"], "notFound");
        let json = serde_json::to_value(IslandyError::from(
            crate::services::system::SystemError::Denied,
        ))
        .unwrap();
        assert_eq!(json["kind"], "denied");
    }
}
