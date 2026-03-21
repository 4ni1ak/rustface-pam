use image::GrayImage;
use v4l::buffer::Type;
use v4l::format::FourCC;
use v4l::io::mmap::Stream;
use v4l::io::traits::CaptureStream;
use v4l::video::Capture;
use v4l::Device;

use super::error::CameraError;

pub const DEFAULT_DEVICE: &str = "/dev/video2";

// Y800 and GREY are the same 8-bit grayscale format — some cameras report GREY
fn is_grayscale8(fourcc: &FourCC) -> bool {
    *fourcc == FourCC::new(b"Y800") || *fourcc == FourCC::new(b"GREY")
}

/// Scan /dev/video* and return the path of the first grayscale (IR) camera found.
/// Falls back to DEFAULT_DEVICE if none found.
pub fn auto_detect_ir_camera() -> String {
    // Collect and sort /dev/video0..N
    let mut candidates: Vec<String> = (0..16)
        .map(|i| format!("/dev/video{i}"))
        .filter(|p| std::path::Path::new(p).exists())
        .collect();
    candidates.sort();

    for path in &candidates {
        if let Ok(dev) = Device::with_path(path) {
            if let Ok(fmt) = dev.format() {
                if is_grayscale8(&fmt.fourcc) {
                    log::info!("Auto-detected IR camera: {path} ({:?} {}x{})", fmt.fourcc, fmt.width, fmt.height);
                    return path.clone();
                }
            }
        }
    }

    log::warn!("No grayscale IR camera found, falling back to {DEFAULT_DEVICE}");
    DEFAULT_DEVICE.to_owned()
}

/// IR camera wrapper — file descriptor managed via RAII.
///
/// `Device` is held here; the stream is opened and closed per capture
/// (fast enough for PAM auth flow).
pub struct IrCamera {
    device: Device,
    pub width: u32,
    pub height: u32,
}

impl IrCamera {
    pub fn open(path: &str) -> Result<Self, CameraError> {
        let device = Device::with_path(path).map_err(|e| CameraError::Open {
            path: path.to_owned(),
            source: Box::new(e),
        })?;

        // Format check — must be Y800 or GREY (8-bit grayscale)
        let fmt = device
            .format()
            .map_err(|e| CameraError::FormatSet(e.to_string()))?;

        if !is_grayscale8(&fmt.fourcc) {
            return Err(CameraError::WrongFormat(format!("{:?}", fmt.fourcc)));
        }

        log::debug!(
            "Camera opened: {} — {}x{} grayscale",
            path,
            fmt.width,
            fmt.height
        );

        Ok(Self {
            device,
            width: fmt.width,
            height: fmt.height,
        })
    }

    /// Capture a single frame from the camera → GrayImage
    ///
    /// Stream is opened and closed per call — sufficient for PAM auth flow.
    pub fn capture_frame(&self) -> Result<GrayImage, CameraError> {
        let mut stream =
            Stream::with_buffers(&self.device, Type::VideoCapture, 4).map_err(|e| {
                CameraError::StreamStart(e.to_string())
            })?;

        // Skip the first frame — camera needs a warm-up; second frame is cleaner
        let _ = stream.next().map_err(|e| CameraError::FrameCapture(e.to_string()))?;
        let (buf, _meta) = stream
            .next()
            .map_err(|e| CameraError::FrameCapture(e.to_string()))?;

        GrayImage::from_raw(self.width, self.height, buf.to_vec())
            .ok_or(CameraError::FrameConversion)
    }

    /// Capture the specified number of frames, return the last one.
    /// Skips `skip` frames for camera warm-up.
    pub fn capture_warmed_frame(&self, skip: u32) -> Result<GrayImage, CameraError> {
        let mut stream =
            Stream::with_buffers(&self.device, Type::VideoCapture, 4).map_err(|e| {
                CameraError::StreamStart(e.to_string())
            })?;

        let total = skip + 1;
        let mut last_buf: Option<Vec<u8>> = None;

        for _ in 0..total {
            let (buf, _) = stream
                .next()
                .map_err(|e| CameraError::FrameCapture(e.to_string()))?;
            last_buf = Some(buf.to_vec());
        }

        let raw = last_buf.unwrap();
        GrayImage::from_raw(self.width, self.height, raw).ok_or(CameraError::FrameConversion)
    }
}

impl Drop for IrCamera {
    fn drop(&mut self) {
        // device Drop impl closes the v4l fd automatically
        log::debug!("Camera closed");
    }
}
