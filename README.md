# rustface-pam

A Linux PAM module that authenticates users via face recognition using an IR camera. Look at your camera to run `sudo` — no password needed.

```
$ sudo apt list --installed
[sudo] Look at the camera...
✓ Authenticated
```

## How It Works

1. **Face Detection** — SeetaFace (rustface crate) detects faces in IR camera frames
2. **Embedding** — ArcFace (ONNX, w600k_r50) generates a 512-dimensional face vector
3. **Matching** — Cosine similarity is compared against your enrolled embedding
4. **Online Learning** — Each successful auth slightly updates your stored embedding to adapt to lighting and appearance changes over time

## Requirements

- Linux x86_64
- IR camera outputting 8-bit grayscale (Y800 or GREY FourCC), default `/dev/video2`
- Rust toolchain (stable)
- ~200 MB disk space for model files

## Installation

### 1. Clone and build

```bash
git clone https://github.com/pervane/rustface-pam
cd rustface-pam
cargo build --release
```

### 2. Download model files

```bash
sudo bash download-models.sh
```

This downloads:
- `seeta_fd_frontal_v1.0.bin` — SeetaFace detection model (~1.2 MB)
- `arcface.onnx` — ArcFace embedding model, w600k_r50 (~167 MB)

### 3. Install the PAM module

```bash
sudo bash install.sh
```

This:
- Copies `libpam_rustface.so` → `/usr/lib/security/pam_rustface.so`
- Creates `/etc/rustface/faces/` and `/etc/rustface/models/`
- Adds the auth line to `/etc/pam.d/sudo`

### 4. Enroll your face

```bash
sudo rustface-enroll $USER
```

Look directly at the camera when prompted. Your face embedding is stored at `/etc/rustface/faces/<username>.bin`.

### 5. Test

```bash
sudo -k && sudo echo "hello"
```

Look at the camera. Sudo should succeed without asking for a password.

## PAM Configuration

`/etc/pam.d/sudo` entry added by `install.sh`:

```
auth  sufficient  /usr/lib/security/pam_rustface.so  threshold=0.8  timeout=3
auth  include  system-auth
```

### Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `threshold` | `0.8` | Minimum cosine similarity required to authenticate |
| `timeout` | `3` | Seconds to wait for a face before giving up |
| `device` | `/dev/video2` | Camera device path |
| `face_model` | `/etc/rustface/models/seeta_fd_frontal_v1.0.bin` | SeetaFace model path |
| `embed_model` | `/etc/rustface/models/arcface.onnx` | ArcFace model path |

## Scoring

Check your live similarity score to verify recognition is working:

```bash
sudo rustface-sim-test $USER
```

Typical scores:
- **Same person:** 0.75 – 0.95
- **Different person:** 0.05 – 0.25

## Online Learning

On each successful authentication:
- Similarity ≥ 0.8: embedding updated with α = 0.2 (gentle adaptation)
- Similarity ≥ 0.9: embedding updated with α = 0.4 (stronger adaptation)

This lets the model gradually adapt to changes in lighting, glasses, haircut, etc.

## Inference Backends

### CPU (default)

```bash
cargo build --release
sudo bash install.sh
```

### Intel Arc GPU

```bash
cargo build --release --features gpu
sudo bash install.sh --gpu
```

### Intel NPU (Core Ultra AI Boost)

```bash
# Install driver first
yay -S intel-npu-driver openvino

cargo build --release --features npu
sudo bash install.sh --npu
```

## Utilities

| Command | Description |
|---------|-------------|
| `sudo rustface-enroll [user]` | Enroll a face |
| `sudo rustface-sim-test [user]` | Show live similarity scores |
| `cargo run --release --bin camera-test -- /dev/video2 out.png` | Capture a test frame |

## Security Notes

- On any failure (no face, low similarity, camera error) the module returns `PAM_IGNORE` — sudo falls through to password authentication
- `PAM_AUTH_ERR` is never returned, so `faillock` is never triggered
- Enrolled embeddings are stored in `/etc/rustface/faces/` (root:root, mode 700)
- The module is panic-safe: any panic returns `PAM_IGNORE`

## Uninstall

```bash
sudo bash uninstall-test.sh
```

Removes the PAM module and restores the original `/etc/pam.d/sudo`.

## Building

```bash
# Default (CPU)
cargo build --release

# With GPU support
cargo build --release --features gpu

# With NPU support
cargo build --release --features npu

# Test mode (always returns PAM_SUCCESS — development only)
cargo build --release --features pam-test

# Run unit tests
cargo test
```

## License

MIT
