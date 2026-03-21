# Contributing to rustface-pam

Thank you for your interest in contributing! This document explains how to get involved.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/rustface-pam`
3. Create a branch: `git checkout -b feature/my-feature`
4. Make your changes
5. Run tests: `cargo test`
6. Submit a pull request

## Development Setup

### Requirements

- Rust stable toolchain
- Linux x86_64
- IR camera at `/dev/video2` (for integration testing)
- `libpam-dev`, `libv4l-dev` (for linking)

### Build

```bash
cargo build               # debug
cargo build --release     # release
cargo test                # unit tests
cargo clippy              # linter
cargo fmt                 # formatter
```

### Test without a camera

Unit tests in `src/face/compare.rs` run without hardware. Integration tests (camera, enroll, PAM) require a physical IR camera.

## Project Structure

```
src/
├── lib.rs          # PAM entry point, auth flow, online learning
├── error.rs        # Unified error type
├── pam/            # PAM C FFI bindings (constants, handle)
├── camera/         # V4L2 IR camera capture
├── face/           # Detection (SeetaFace), embedding (ArcFace), comparison
├── storage/        # Embedding serialization (binary format)
└── bin/
    ├── enroll.rs       # rustface-enroll CLI
    ├── camera_test.rs  # camera-test utility
    └── sim_test.rs     # rustface-sim-test utility
```

## Code Style

- Follow standard Rust conventions (`cargo fmt`, `cargo clippy`)
- All comments and strings must be in **English**
- Error paths must return `PAM_IGNORE`, never `PAM_AUTH_ERR`
- No `unwrap()` in library code — use `?` or explicit error handling

## Pull Request Guidelines

- Keep PRs focused — one feature or fix per PR
- Add or update tests where applicable
- Update `README.md` if behavior or configuration changes
- Describe what changed and why in the PR description

## Reporting Issues

Open an issue with:
- OS and kernel version
- Camera model and FourCC format (`v4l2-ctl --list-formats-ext -d /dev/videoN`)
- Relevant log output
- Steps to reproduce

## Security Issues

Do not open public issues for security vulnerabilities. Email the maintainer directly.
