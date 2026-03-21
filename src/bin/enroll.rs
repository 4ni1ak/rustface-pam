/// rustface-enroll — face enrollment CLI
///
/// Usage:
///   sudo rustface-enroll [username] [model_dir]
///
/// Example:
///   sudo rustface-enroll alice /etc/rustface/models
///
/// Requirements:
///   - /etc/rustface/faces/ must exist with root:root 700 permissions
///   - /etc/rustface/models/seeta_fd_frontal_v1.0.bin
///   - /etc/rustface/models/arcface.onnx

use std::path::PathBuf;

use pam_rustface::{
    camera::IrCamera,
    face::{FaceDetector, FaceEmbedder},
    storage::FaceStore,
};

const FACE_MODEL: &str = "seeta_fd_frontal_v1.0.bin";
const EMBED_MODEL: &str = "arcface.onnx";
const WARMUP_FRAMES: u32 = 5;
const DETECT_RETRIES: u32 = 10;

fn main() {
    // Enrollment requires root
    if unsafe { libc_getuid() } != 0 {
        eprintln!("[enroll] ERROR: run as root (sudo rustface-enroll)");
        std::process::exit(1);
    }

    let username = std::env::args()
        .nth(1)
        .unwrap_or_else(|| {
            // Fall back to SUDO_USER env var
            std::env::var("SUDO_USER").unwrap_or_else(|_| {
                eprintln!("[enroll] ERROR: specify a username: sudo rustface-enroll <username>");
                std::process::exit(1);
            })
        });

    let model_dir = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "/etc/rustface/models".to_owned());

    let face_model_path = PathBuf::from(&model_dir).join(FACE_MODEL);
    let embed_model_path = PathBuf::from(&model_dir).join(EMBED_MODEL);

    eprintln!("[enroll] User: {username}");
    eprintln!("[enroll] Face model: {}", face_model_path.display());
    eprintln!("[enroll] Embed model: {}", embed_model_path.display());

    // Check model files
    if !face_model_path.exists() {
        eprintln!("[enroll] ERROR: {} not found", face_model_path.display());
        eprintln!("[enroll] Run: sudo bash download-models.sh");
        std::process::exit(1);
    }
    if !embed_model_path.exists() {
        eprintln!("[enroll] ERROR: {} not found", embed_model_path.display());
        eprintln!("[enroll] Run: sudo bash download-models.sh");
        std::process::exit(1);
    }

    // Load detector and embedder
    eprintln!("[enroll] Loading models...");
    let mut detector = FaceDetector::load(face_model_path.to_str().unwrap())
        .unwrap_or_else(|e| { eprintln!("[enroll] ERROR loading detector: {e}"); std::process::exit(1); });

    let mut embedder = FaceEmbedder::load(embed_model_path.to_str().unwrap())
        .unwrap_or_else(|e| { eprintln!("[enroll] ERROR loading embedder: {e}"); std::process::exit(1); });

    // Open camera — auto-detect IR camera if not specified
    let device = std::env::args().nth(3)
        .unwrap_or_else(|| pam_rustface::camera::auto_detect_ir_camera());
    eprintln!("[enroll] Camera: {device}");
    let camera = IrCamera::open(&device)
        .unwrap_or_else(|e| { eprintln!("[enroll] ERROR opening camera: {e}"); std::process::exit(1); });

    eprintln!("[enroll] Camera ready: {}x{}", camera.width, camera.height);
    eprintln!("[enroll] Look at the camera... ({DETECT_RETRIES} attempts)");

    // Face detection loop
    let embedding = 'detect: {
        for attempt in 1..=DETECT_RETRIES {
            eprintln!("[enroll] Attempt {attempt}/{DETECT_RETRIES}...");

            let frame = match camera.capture_warmed_frame(WARMUP_FRAMES) {
                Ok(f) => f,
                Err(e) => { eprintln!("[enroll] Frame error: {e}"); continue; }
            };

            let Some(region) = detector.detect(&frame) else {
                eprintln!("[enroll] No face found");
                continue;
            };

            eprintln!(
                "[enroll] Face found: {}x{} @ ({},{}) — confidence: {:.2}",
                region.width, region.height, region.x, region.y, region.confidence
            );

            match embedder.embed(&frame, &region) {
                Ok(emb) => {
                    eprintln!("[enroll] Embedding extracted — dim: {}", emb.dim());
                    break 'detect emb;
                }
                Err(e) => { eprintln!("[enroll] Embedding error: {e}"); continue; }
            }
        }

        eprintln!("[enroll] ERROR: no face detected after {DETECT_RETRIES} attempts");
        std::process::exit(1);
    };

    // Save
    let store = FaceStore::default();
    match store.save(&username, &embedding) {
        Ok(()) => {
            eprintln!("[enroll] Enrolled successfully: /etc/rustface/faces/{username}.bin");
            eprintln!("[enroll] You can now authenticate with sudo by looking at the camera.");
        }
        Err(e) => {
            eprintln!("[enroll] ERROR saving embedding: {e}");
            std::process::exit(1);
        }
    }
}

// getuid syscall without libc dependency
#[cfg(target_os = "linux")]
unsafe fn libc_getuid() -> u32 {
    let uid: u32;
    std::arch::asm!(
        "syscall",
        in("rax") 102u64, // SYS_getuid
        lateout("rax") uid,
        options(nostack)
    );
    uid
}
