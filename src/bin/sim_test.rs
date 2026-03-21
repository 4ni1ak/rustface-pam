/// rustface-sim-test — show live cosine similarity score against enrolled embedding
///
/// Usage: sudo rustface-sim-test [username]
///
/// Useful for tuning the threshold and verifying that face recognition works.

use std::path::PathBuf;
use pam_rustface::{
    camera::IrCamera,
    face::{cosine_similarity, FaceDetector, FaceEmbedder},
    storage::FaceStore,
};

const THRESHOLD: f32 = 0.8;

fn main() {
    let username = std::env::args().nth(1).unwrap_or_else(|| "pervane".to_owned());
    let model_dir = "/etc/rustface/models";

    let face_model = PathBuf::from(model_dir).join("seeta_fd_frontal_v1.0.bin");
    let embed_model = PathBuf::from(model_dir).join("arcface.onnx");

    eprintln!("[sim-test] Loading models...");
    let mut detector = FaceDetector::load(face_model.to_str().unwrap())
        .expect("Failed to load face detector");
    let mut embedder = FaceEmbedder::load(embed_model.to_str().unwrap())
        .expect("Failed to load embedder");

    eprintln!("[sim-test] Loading enrolled embedding for '{username}'...");
    let store = FaceStore::default();
    let stored = store.load(&username).expect("No enrolled face found — run rustface-enroll first");

    eprintln!("[sim-test] Opening camera...");
    let camera = IrCamera::open("/dev/video2").expect("Failed to open camera");

    eprintln!("[sim-test] Look at the camera...");
    for attempt in 1..=5 {
        let frame = camera.capture_warmed_frame(3).expect("Failed to capture frame");
        let Some(region) = detector.detect(&frame) else {
            eprintln!("[sim-test] Attempt {attempt}/5: no face detected");
            continue;
        };
        let live = embedder.embed(&frame, &region).expect("Embedding failed");
        let sim = cosine_similarity(&live, &stored);
        println!(
            "[sim-test] Attempt {attempt}/5: similarity = {sim:.4}  (threshold={THRESHOLD} → {})",
            if sim >= THRESHOLD { "PASS ✓" } else { "REJECT ✗" }
        );
    }
}
