use crate::camera::CameraError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RustfaceError {
    #[error("Failed to open camera: {path} — {source}")]
    CameraOpen {
        path: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Failed to capture frame: {0}")]
    CameraCapture(String),

    #[error("Wrong camera format: {0}, expected Y800 or GREY")]
    WrongFormat(String),

    #[error("No face detected")]
    NoFaceDetected,

    #[error("No enrolled face found for user: {username}")]
    NoEnrolledFace { username: String },

    #[error("Failed to read embedding file: {0}")]
    StorageRead(#[from] std::io::Error),

    #[error("ONNX model error: {0}")]
    OnnxError(String),

    #[error("PAM handle error")]
    PamHandle,

    #[error("Failed to get username")]
    NoUsername,
}

impl From<CameraError> for RustfaceError {
    fn from(e: CameraError) -> Self {
        RustfaceError::CameraOpen {
            path: String::new(),
            source: Box::new(std::io::Error::other(e.to_string())),
        }
    }
}
