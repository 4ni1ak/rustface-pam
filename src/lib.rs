pub mod camera;
pub mod face;
pub mod storage;

mod error;
mod pam;

pub use camera::capture::IrCamera;
pub use error::RustfaceError;

pub fn camera_open(path: &str) -> Result<IrCamera, camera::CameraError> {
    IrCamera::open(path)
}

use pam::constants::{PAM_IGNORE, PAM_SUCCESS};
use pam::handle::PamHandle;
use std::os::raw::{c_char, c_int};

/// PAM entry point — signature must match exactly
#[no_mangle]
pub extern "C" fn pam_sm_authenticate(
    pamh: *mut PamHandle,
    flags: c_int,
    argc: c_int,
    argv: *const *const c_char,
) -> c_int {
    let result = std::panic::catch_unwind(|| authenticate_inner(pamh, flags, argc, argv));
    match result {
        Ok(r) => r,
        Err(_) => PAM_IGNORE,
    }
}

#[no_mangle]
pub extern "C" fn pam_sm_setcred(
    _pamh: *mut PamHandle,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    PAM_SUCCESS
}

fn authenticate_inner(
    pamh: *mut PamHandle,
    _flags: c_int,
    argc: c_int,
    argv: *const *const c_char,
) -> c_int {
    #[cfg(feature = "pam-test")]
    {
        log::info!("rustface-pam: pam-test mode — PAM_SUCCESS");
        return PAM_SUCCESS;
    }

    #[allow(unreachable_code)]
    {
        let config = match Config::from_pam_args(argc, argv) {
            Ok(c) => c,
            Err(_) => return PAM_IGNORE,
        };

        let username = match unsafe { pam::handle::get_username(pamh) } {
            Some(u) => u,
            None => return PAM_IGNORE,
        };

        match run_face_auth(&username, &config) {
            Ok(matched) => {
                if matched {
                    log::info!("rustface-pam: {} — PAM_SUCCESS", username);
                    PAM_SUCCESS
                } else {
                    log::info!("rustface-pam: {} — no match, PAM_IGNORE", username);
                    PAM_IGNORE
                }
            }
            Err(e) => {
                log::warn!("rustface-pam: error — {e}");
                PAM_IGNORE
            }
        }
    }
}

// ─── Config ──────────────────────────────────────────────────────────────────

struct Config {
    threshold: f32,
    timeout_secs: u64,
    device: String,
    debug: bool,
    face_model: String,
    embed_model: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            threshold: 0.8,
            timeout_secs: 3,
            device: "auto".to_owned(),
            debug: false,
            face_model: "/etc/rustface/models/seeta_fd_frontal_v1.0.bin".to_owned(),
            embed_model: "/etc/rustface/models/arcface.onnx".to_owned(),
        }
    }
}

impl Config {
    fn from_pam_args(argc: c_int, argv: *const *const c_char) -> Result<Self, ()> {
        use std::ffi::CStr;

        let mut cfg = Config::default();

        if argc <= 0 || argv.is_null() {
            return Ok(cfg);
        }

        for i in 0..argc as usize {
            // SAFETY: PAM argc/argv contract — null-terminated string array
            let arg = unsafe {
                let ptr = *argv.add(i);
                if ptr.is_null() {
                    continue;
                }
                CStr::from_ptr(ptr).to_str().unwrap_or("")
            };

            if let Some(val) = arg.strip_prefix("threshold=") {
                if let Ok(v) = val.parse::<f32>() {
                    cfg.threshold = v;
                }
            } else if let Some(val) = arg.strip_prefix("timeout=") {
                if let Ok(v) = val.parse::<u64>() {
                    cfg.timeout_secs = v;
                }
            } else if let Some(val) = arg.strip_prefix("device=") {
                cfg.device = val.to_owned();
            } else if arg == "debug=true" || arg == "debug" {
                cfg.debug = true;
            } else if let Some(val) = arg.strip_prefix("face_model=") {
                cfg.face_model = val.to_owned();
            } else if let Some(val) = arg.strip_prefix("embed_model=") {
                cfg.embed_model = val.to_owned();
            }
            // unknown argument → ignore (safe)
        }

        Ok(cfg)
    }
}

// ─── Main auth flow ───────────────────────────────────────────────────────────

fn run_face_auth(username: &str, config: &Config) -> Result<bool, RustfaceError> {
    use camera::IrCamera;
    use face::{cosine_similarity, FaceDetector, FaceEmbedder};
    use std::time::{Duration, Instant};
    use storage::FaceStore;

    // 1. Check for enrolled face
    let store = FaceStore::default();
    if !store.exists(username) {
        log::debug!("rustface-pam: no enrolled face for {username}");
        return Ok(false);
    }

    let stored_emb = store.load(username)?;

    // 2. Open camera — "auto" triggers IR camera auto-detection
    let device_path = if config.device == "auto" {
        camera::capture::auto_detect_ir_camera()
    } else {
        config.device.clone()
    };
    let camera = IrCamera::open(&device_path)?;

    // 3. Load face detector and embedder
    let mut detector = FaceDetector::load(&config.face_model)
        .map_err(|e| RustfaceError::OnnxError(e.to_string()))?;

    let mut embedder = FaceEmbedder::load(&config.embed_model)?;

    // 4. Timeout loop — give the user time to look at the camera
    let deadline = Instant::now() + Duration::from_secs(config.timeout_secs);
    const MAX_FRAMES: u32 = 30;

    for _ in 0..MAX_FRAMES {
        if Instant::now() > deadline {
            log::debug!("rustface-pam: timeout");
            return Ok(false);
        }

        let frame = match camera.capture_frame() {
            Ok(f) => f,
            Err(e) => {
                log::warn!("rustface-pam: frame error: {e}");
                continue;
            }
        };

        let Some(region) = detector.detect(&frame) else {
            continue;
        };

        let live_emb = match embedder.embed(&frame, &region) {
            Ok(e) => e,
            Err(e) => {
                log::warn!("rustface-pam: embed error: {e}");
                continue;
            }
        };

        let sim = cosine_similarity(&live_emb, &stored_emb);
        log::debug!(
            "rustface-pam: similarity={sim:.3} threshold={}",
            config.threshold
        );

        if sim >= config.threshold {
            // Online learning: update stored embedding on high-confidence matches
            // 0.8–0.9 → gentle update (α=0.2), 0.9+ → stronger update (α=0.4)
            let alpha = if sim >= 0.9 { 0.4_f32 } else { 0.2_f32 };
            let updated = blend_embeddings(&stored_emb, &live_emb, alpha);
            let _ = store.save(username, &updated);
            log::debug!("rustface-pam: embedding updated (sim={sim:.3} α={alpha})");

            return Ok(true);
        }
    }

    Ok(false)
}

/// Weighted blend of two embeddings, L2-normalized.
/// alpha = weight of the new embedding (0.0 = keep old, 1.0 = replace with new)
fn blend_embeddings(
    old: &face::embed::Embedding,
    new: &face::embed::Embedding,
    alpha: f32,
) -> face::embed::Embedding {
    let blended: Vec<f32> = old
        .as_slice()
        .iter()
        .zip(new.as_slice().iter())
        .map(|(o, n)| (1.0 - alpha) * o + alpha * n)
        .collect();
    // L2 normalize — required for cosine similarity to stay consistent
    let norm: f32 = blended.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        face::embed::Embedding::from_slice(&blended.iter().map(|x| x / norm).collect::<Vec<_>>())
    } else {
        face::embed::Embedding::from_slice(&blended)
    }
}
