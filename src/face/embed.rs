use image::imageops;
use image::GrayImage;
use ndarray::Array;
use ort::session::Session;
use ort::value::Tensor;

use crate::error::RustfaceError;
use crate::face::detect::FaceRegion;

const FACE_SIZE: u32 = 112; // ArcFace standard input size

/// Face embedding vector — dimension depends on model (512 for ArcFace, 128 for FaceNet)
#[derive(Debug, Clone)]
pub struct Embedding(pub Vec<f32>);

impl Embedding {
    pub fn from_slice(slice: &[f32]) -> Self {
        Self(slice.to_vec())
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }

    pub fn dim(&self) -> usize {
        self.0.len()
    }
}

/// ONNX-based face embedding extractor (ArcFace / FaceNet)
pub struct FaceEmbedder {
    session: Session,
    input_name: String,
}

impl FaceEmbedder {
    /// Load from an ONNX model file
    /// model_path: /etc/rustface/models/arcface.onnx
    pub fn load(model_path: &str) -> Result<Self, RustfaceError> {
        use crate::face::backend::{build_session, InferenceDevice};

        let device = InferenceDevice::best_available();
        let session = build_session(model_path, device)
            .map_err(|e| RustfaceError::OnnxError(e.to_string()))?;

        let input_name = session
            .inputs()
            .first()
            .map(|i| i.name().to_owned())
            .unwrap_or_else(|| "input".to_owned());

        log::debug!("FaceEmbedder loaded — device: {device:?}, input: {input_name}");

        Ok(Self {
            session,
            input_name,
        })
    }

    /// Crop the face region from the image, normalize, and extract the embedding
    pub fn embed(
        &mut self,
        image: &GrayImage,
        region: &FaceRegion,
    ) -> Result<Embedding, RustfaceError> {
        // 1. Crop face region
        let crop =
            imageops::crop_imm(image, region.x, region.y, region.width, region.height).to_image();

        // 2. Resize to 112x112
        let resized = imageops::resize(&crop, FACE_SIZE, FACE_SIZE, imageops::FilterType::Lanczos3);

        // 3. Normalize: [0,255] → [-1.0, 1.0] (ArcFace standard)
        // ArcFace expects RGB input; replicate the IR grayscale channel 3 times [R=G=B=gray]
        // NCHW layout: all R pixels first, then G, then B
        let pixels_norm: Vec<f32> = resized
            .pixels()
            .map(|p| (p[0] as f32 / 127.5) - 1.0)
            .collect();
        let mut normalized = Vec::with_capacity(3 * pixels_norm.len());
        normalized.extend_from_slice(&pixels_norm); // channel 0 (R)
        normalized.extend_from_slice(&pixels_norm); // channel 1 (G)
        normalized.extend_from_slice(&pixels_norm); // channel 2 (B)

        // 4. Tensor: shape [1, 3, 112, 112] — batch, channels (RGB → gray replicated), H, W
        let shape = [1usize, 3, FACE_SIZE as usize, FACE_SIZE as usize];
        let array = Array::from_shape_vec(shape, normalized)
            .map_err(|e| RustfaceError::OnnxError(e.to_string()))?;

        let tensor = Tensor::from_array(array.into_dyn())
            .map_err(|e| RustfaceError::OnnxError(e.to_string()))?;

        // 5. Inference — ort::inputs! returns Vec directly (not Result)
        let inputs = ort::inputs![self.input_name.as_str() => tensor];

        let outputs = self
            .session
            .run(inputs)
            .map_err(|e| RustfaceError::OnnxError(e.to_string()))?;

        // 6. Extract first output tensor → (_shape, &[f32])
        let output = outputs
            .values()
            .next()
            .ok_or_else(|| RustfaceError::OnnxError("Model output is empty".to_owned()))?;

        let (_shape, data) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| RustfaceError::OnnxError(e.to_string()))?;

        let embedding = Embedding::from_slice(data);
        log::debug!("Embedding extracted — dim: {}", embedding.dim());

        Ok(embedding)
    }
}
