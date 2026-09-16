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
        Err(e) => {
            let _ = syslog::init(
                syslog::Facility::LOG_AUTH,
                log::LevelFilter::Debug,
                Some("rustface-pam"),
            );
            log::error!("PAM module panicked: {:?}", e);
            PAM_IGNORE
        }
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
    // Initialize syslog
    let _ = syslog::init(
        syslog::Facility::LOG_AUTH,
        log::LevelFilter::Debug,
        Some("rustface-pam"),
    );

    #[cfg(feature = "pam-test")]
    {
        log::info!("pam-test mode — PAM_SUCCESS");
        return PAM_SUCCESS;
    }

    #[allow(unreachable_code)]
    {
        let config = match Config::from_pam_args(argc, argv) {
            Ok(c) => c,
            Err(_) => return PAM_IGNORE,
        };

        // Uptime guard: on fresh boot skip face-auth and fall through to password
        if config.min_uptime_secs > 0 {
            match read_uptime_secs() {
                Some(up) if up < config.min_uptime_secs => {
                    log::info!(
                        "rustface-pam: uptime {}s < min_uptime {}s — PAM_IGNORE (fresh boot)",
                        up,
                        config.min_uptime_secs
                    );
                    return PAM_IGNORE;
                }
                Some(up) => log::debug!(
                    "uptime {}s >= min_uptime {}s, proceeding with face auth",
                    up,
                    config.min_uptime_secs
                ),
                None => {
                    log::warn!("rustface-pam: could not read /proc/uptime — PAM_IGNORE");
                    return PAM_IGNORE;
                }
            }
        }

        let username = match unsafe { pam::handle::get_username(pamh) } {
            Some(u) => {
                log::debug!("detected username: {u}");
                u
            },
            None => {
                log::warn!("could not get username from PAM handle");
                return PAM_IGNORE;
            }
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
    min_uptime_secs: u64, // 0 = disabled; login stacks set e.g. 120
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
            min_uptime_secs: 0,
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
            } else if let Some(val) = arg.strip_prefix("min_uptime=") {
                if let Ok(v) = val.parse::<u64>() {
                    cfg.min_uptime_secs = v;
                }
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

    log::info!("Starting face auth for user: {}", username);

    // 1. Check for enrolled face
    let store = FaceStore::default();
    if !store.exists(username) {
        log::warn!("No enrolled face for user: {}. Please run 'sudo rustface-enroll {}' first.", username, username);
        return Ok(false);
    }

    log::debug!("Loading stored embedding for user: {}", username);
    let stored_emb = store.load(username)?;

    // 2. Open camera — "auto" triggers IR camera auto-detection
    let device_path = if config.device == "auto" {
        log::debug!("Auto-detecting IR camera...");
        camera::capture::auto_detect_ir_camera()
    } else {
        config.device.clone()
    };
    log::info!("Opening camera device: {}", device_path);
    let camera = IrCamera::open(&device_path)?;

    // 3. Load face detector and embedder
    log::debug!("Loading face detector from: {}", config.face_model);
    let mut detector = FaceDetector::load(&config.face_model)
        .map_err(|e| RustfaceError::OnnxError(e.to_string()))?;

    log::debug!("Loading face embedder from: {}", config.embed_model);
    let mut embedder = FaceEmbedder::load(&config.embed_model)?;

    // 4. Timeout loop — give the user time to look at the camera
    let deadline = Instant::now() + Duration::from_secs(config.timeout_secs);
    const MAX_FRAMES: u32 = 30;

    log::info!("Starting capture loop (timeout: {}s)...", config.timeout_secs);

    for i in 0..MAX_FRAMES {
        if Instant::now() > deadline {
            log::warn!("Face auth timed out for user: {}", username);
            return Ok(false);
        }

        let frame = match camera.capture_frame() {
            Ok(f) => f,
            Err(e) => {
                log::warn!("Frame capture error: {e}");
                continue;
            }
        };

        log::debug!("Processing frame #{}", i);

        let Some(region) = detector.detect(&frame) else {
            continue;
        };

        log::info!("Face detected, computing embedding...");

        let live_emb = match embedder.embed(&frame, &region) {
            Ok(e) => e,
            Err(e) => {
                log::warn!("Embedding computation error: {e}");
                continue;
            }
        };

        let sim = cosine_similarity(&live_emb, &stored_emb);
        log::info!(
            "Similarity: {:.3} (threshold: {})",
            sim,
            config.threshold
        );

        if sim >= config.threshold {
            log::info!("Face match successful! Similarity: {:.3}", sim);
            // Online learning: update stored embedding on high-confidence matches
            // 0.8–0.9 → gentle update (α=0.2), 0.9+ → stronger update (α=0.4)
            let alpha = if sim >= 0.9 { 0.4_f32 } else { 0.2_f32 };
            let updated = blend_embeddings(&stored_emb, &live_emb, alpha);
            let _ = store.save(username, &updated);
            log::debug!("Updated stored embedding for user: {} (α={})", username, alpha);

            return Ok(true);
        }
    }

    log::warn!("No face match found within {} frames.", MAX_FRAMES);
    Ok(false)
}

/// Seconds elapsed since system boot, read from /proc/uptime.
fn read_uptime_secs() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/uptime").ok()?;
    let first = content.split_whitespace().next()?;
    first.parse::<f64>().ok().map(|s| s as u64)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_uptime_returns_nonzero() {
        let up = read_uptime_secs();
        assert!(up.is_some(), "could not read /proc/uptime");
        assert!(up.unwrap() > 0, "uptime should be > 0");
    }

    #[test]
    fn config_parses_min_uptime() {
        use std::ffi::CString;

        let arg = CString::new("min_uptime=120").unwrap();
        let ptrs: Vec<*const std::os::raw::c_char> = vec![arg.as_ptr()];
        let cfg = Config::from_pam_args(1, ptrs.as_ptr()).unwrap();
        assert_eq!(cfg.min_uptime_secs, 120);
    }

    #[test]
    fn config_default_min_uptime_is_zero() {
        let cfg = Config::default();
        assert_eq!(cfg.min_uptime_secs, 0);
    }
}
