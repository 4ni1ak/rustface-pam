/// camera-test — capture a frame from the IR camera and save it as PNG
/// Usage: cargo run --bin camera-test [/dev/video2] [output.png]

fn main() {
    eprintln!("[camera-test] Starting...");

    let device_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| pam_rustface::camera::auto_detect_ir_camera());
    let output_path = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "frame.png".to_owned());

    eprintln!("[camera-test] Device: {device_path}");

    let camera = match pam_rustface::camera_open(&device_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[camera-test] ERROR — failed to open camera: {e}");
            std::process::exit(1);
        }
    };

    eprintln!("[camera-test] Resolution: {}x{}", camera.width, camera.height);
    eprintln!("[camera-test] Capturing frame (3 warm-up frames)...");

    let frame: image::GrayImage = match camera.capture_warmed_frame(3) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[camera-test] ERROR — failed to capture frame: {e}");
            std::process::exit(1);
        }
    };

    eprintln!("[camera-test] Frame captured: {}x{}", frame.width(), frame.height());

    if let Err(e) = frame.save(&output_path) {
        eprintln!("[camera-test] ERROR — failed to save PNG: {e}");
        std::process::exit(1);
    }

    eprintln!("[camera-test] Saved: {output_path}");
}
