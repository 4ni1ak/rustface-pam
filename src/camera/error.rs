use thiserror::Error;

#[derive(Debug, Error)]
pub enum CameraError {
    #[error("Failed to open device: {path} — {source}")]
    Open {
        path: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Failed to set format: {0}")]
    FormatSet(String),

    #[error("Wrong format: {0}, expected Y800 or GREY")]
    WrongFormat(String),

    #[error("Failed to start stream: {0}")]
    StreamStart(String),

    #[error("Failed to capture frame: {0}")]
    FrameCapture(String),

    #[error("Frame conversion failed — buffer size mismatch")]
    FrameConversion,
}
