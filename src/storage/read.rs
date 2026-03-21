use std::io::Read;
use std::path::Path;

use crate::error::RustfaceError;
use crate::face::embed::Embedding;

/// Read an embedding from file — dynamic size (4-byte header + N*4 bytes of f32)
///
/// Format:
///   [u32 little-endian: dimension N] [f32 x N]
pub fn read_embedding(path: &Path) -> Result<Embedding, RustfaceError> {
    let mut file = std::fs::File::open(path)?;

    // First 4 bytes: dimension
    let mut size_buf = [0u8; 4];
    file.read_exact(&mut size_buf)?;
    let n = u32::from_le_bytes(size_buf) as usize;

    if n == 0 || n > 4096 {
        return Err(RustfaceError::OnnxError(format!(
            "Invalid embedding dimension: {n}"
        )));
    }

    let mut data_buf = vec![0u8; n * 4];
    file.read_exact(&mut data_buf)?;

    let floats: Vec<f32> = data_buf
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();

    Ok(Embedding::from_slice(&floats))
}
