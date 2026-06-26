use crate::text::FontError;
use std::fmt;
use std::io;

/// Crate-owned error type for engine resource creation and asset-loading failures.
///
/// This type intentionally avoids exposing backend-specific error types in public APIs where a
/// crate-owned error is sufficient.
///
/// ```no_run
/// # fn load_asset() -> Result<Vec<u8>, plutonium_engine::EngineError> {
/// let bytes = std::fs::read("examples/media/roboto.ttf")?;
/// Ok(bytes)
/// # }
/// ```
#[derive(Clone, Debug)]
pub enum EngineError {
    /// A filesystem or stream read/write failed.
    IoError {
        kind: io::ErrorKind,
        message: String,
    },
    /// Font loading, metadata parsing, or atlas generation failed.
    FontError(FontError),
    /// Image decoding failed.
    ImageDecodeError(String),
    /// Input resource data was malformed or unsupported.
    InvalidResource(String),
    /// GPU texture creation failed.
    TextureCreationError(String),
    /// GPU buffer creation failed.
    BufferCreationError(String),
    /// GPU shader or pipeline creation failed.
    PipelineCreationError(String),
    /// No compatible adapter was available.
    AdapterUnavailable,
    /// Requesting a GPU device failed.
    DeviceRequestError(String),
    /// Surface creation or configuration failed.
    SurfaceError(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IoError { message, .. } => write!(f, "I/O error: {message}"),
            Self::FontError(err) => write!(f, "font error: {err}"),
            Self::ImageDecodeError(message) => write!(f, "image decode error: {message}"),
            Self::InvalidResource(message) => write!(f, "invalid resource: {message}"),
            Self::TextureCreationError(message) => write!(f, "texture creation error: {message}"),
            Self::BufferCreationError(message) => write!(f, "buffer creation error: {message}"),
            Self::PipelineCreationError(message) => write!(f, "pipeline creation error: {message}"),
            Self::AdapterUnavailable => write!(f, "no compatible GPU adapter available"),
            Self::DeviceRequestError(message) => write!(f, "device request error: {message}"),
            Self::SurfaceError(message) => write!(f, "surface error: {message}"),
        }
    }
}

impl std::error::Error for EngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::FontError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for EngineError {
    fn from(err: io::Error) -> Self {
        Self::IoError {
            kind: err.kind(),
            message: err.to_string(),
        }
    }
}

impl From<FontError> for EngineError {
    fn from(err: FontError) -> Self {
        Self::FontError(err)
    }
}

#[cfg(test)]
mod tests {
    use super::EngineError;
    use crate::text::FontError;
    use std::error::Error;
    use std::io;

    #[test]
    fn missing_asset_io_converts_to_cloneable_errors_without_panicking() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "missing test asset");

        let font_err = FontError::from(io_err);
        let cloned_font_err = font_err.clone();
        assert!(matches!(
            cloned_font_err,
            FontError::IoError {
                kind: io::ErrorKind::NotFound,
                ..
            }
        ));

        let engine_err = EngineError::from(font_err);
        assert!(engine_err.source().is_some());
        assert!(engine_err.to_string().contains("font error"));
    }
}
