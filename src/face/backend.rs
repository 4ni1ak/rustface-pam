/// Inference backend selection
///
/// Priority: NPU → GPU → CPU
/// - NPU: Intel AI Boost (Core Ultra) — lowest power, fastest AI inference
/// - GPU: Intel Arc (OpenCL) — mid power
/// - CPU: Fallback — always works
///
/// NPU setup:
///   pacman -S intel-npu-driver openvino
///   cargo build --features npu
///
/// GPU setup:
///   pacman -S openvino
///   cargo build --features gpu

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InferenceDevice {
    Npu,
    Gpu,
    Cpu,
}

impl InferenceDevice {
    /// Select the best available device based on compile-time feature flags
    pub fn best_available() -> Self {
        if cfg!(feature = "npu") {
            log::info!("rustface: inference backend = Intel NPU (AI Boost)");
            Self::Npu
        } else if cfg!(feature = "gpu") {
            log::info!("rustface: inference backend = Intel Arc GPU (OpenCL)");
            Self::Gpu
        } else {
            log::info!("rustface: inference backend = CPU");
            Self::Cpu
        }
    }
}

/// Build an ONNX session from a model file, selecting the execution provider for the given device
pub fn build_session(
    model_path: &str,
    device: InferenceDevice,
) -> Result<ort::session::Session, ort::Error> {
    let mut builder = ort::session::Session::builder()?;

    #[cfg(any(feature = "npu", feature = "gpu"))]
    let builder = {
        let device_str = match device {
            InferenceDevice::Npu => "NPU",
            InferenceDevice::Gpu => "GPU",
            InferenceDevice::Cpu => "CPU",
        };
        let ep = ort::ep::OpenVINO::default()
            .with_device_type(device_str)
            .build();
        builder.with_execution_providers([ep])?
    };

    let _ = device; // unused in CPU mode
    builder.commit_from_file(model_path)
}
