pub mod capture;
pub mod error;

pub use capture::{auto_detect_ir_camera, IrCamera, DEFAULT_DEVICE};
pub use error::CameraError;
