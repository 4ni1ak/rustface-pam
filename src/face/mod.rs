pub mod backend;
pub mod compare;
pub mod detect;
pub mod embed;

pub use backend::{build_session, InferenceDevice};
pub use compare::{cosine_similarity, is_match};
pub use detect::{FaceDetector, FaceRegion};
pub use embed::{Embedding, FaceEmbedder};
