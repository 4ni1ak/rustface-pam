use std::io::Write;
use std::path::Path;

use crate::error::RustfaceError;
use crate::face::embed::Embedding;

/// Write an embedding to file
///
/// Format:
///   [u32 little-endian: dimension N] [f32 x N]
pub fn write_embedding(path: &Path, embedding: &Embedding) -> Result<(), RustfaceError> {
    let slice = embedding.as_slice();
    let n = slice.len() as u32;

    let mut file = std::fs::File::create(path)?;

    // Dimension header
    file.write_all(&n.to_le_bytes())?;

    // Float data
    for val in slice {
        file.write_all(&val.to_le_bytes())?;
    }

    Ok(())
}
