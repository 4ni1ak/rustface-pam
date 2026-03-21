use image::GrayImage;
use rustface::{create_detector, ImageData};

use crate::error::RustfaceError;

/// Detected face region
#[derive(Debug, Clone)]
pub struct FaceRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub confidence: f32,
}

/// Face detector backed by rustface (SeetaFace)
pub struct FaceDetector {
    inner: Box<dyn rustface::Detector>,
}

impl FaceDetector {
    /// Load from a SeetaFace model file
    /// Model path: /etc/rustface/models/seeta_fd_frontal_v1.0.bin
    pub fn load(model_path: &str) -> Result<Self, RustfaceError> {
        let mut detector = create_detector(model_path).map_err(|e| {
            RustfaceError::OnnxError(format!("Failed to load rustface model: {e}"))
        })?;

        // Settings tuned for IR camera input
        detector.set_min_face_size(40);          // minimum face size in px (smaller = more sensitive)
        detector.set_score_thresh(1.5);          // confidence threshold (lower for IR)
        detector.set_pyramid_scale_factor(0.8);  // pyramid scale step
        detector.set_slide_window_step(4, 4);    // sliding window step

        Ok(Self { inner: detector })
    }

    /// Return the most confident face in the image, or None if no face found
    pub fn detect(&mut self, image: &GrayImage) -> Option<FaceRegion> {
        let (w, h) = (image.width(), image.height());
        let raw = image.as_raw();

        // rustface expects grayscale, 1 channel
        let img_data = ImageData::new(raw, w, h);
        let faces = self.inner.detect(&img_data);

        // Select the highest-scoring face
        faces
            .into_iter()
            .max_by(|a, b| a.score().partial_cmp(&b.score()).unwrap_or(std::cmp::Ordering::Equal))
            .map(|face| {
                let bbox = face.bbox();
                // bbox.x/y may be negative near camera edges — clamp to zero
                let x = bbox.x().max(0) as u32;
                let y = bbox.y().max(0) as u32;
                let bw = bbox.width().min(w.saturating_sub(x));
                let bh = bbox.height().min(h.saturating_sub(y));

                FaceRegion {
                    x,
                    y,
                    width: bw,
                    height: bh,
                    confidence: face.score() as f32,
                }
            })
    }
}
