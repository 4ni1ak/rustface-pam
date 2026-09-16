use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::error::RustfaceError;
use crate::face::embed::Embedding;

/// Write an embedding to file
///
/// Format:
///   [u32 little-endian: dimension N] [f32 x N]
///
/// The file is a biometric template, so it's locked to 0600 (owner-only) right
/// after writing, and best-effort chowned to the owning user (derived from the
/// `<username>.bin` filename) so an unprivileged PAM client — e.g. the screen
/// locker, which runs as the user rather than root — can still read its own
/// template without other local users being able to.
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

    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    drop(file);

    if let Some(username) = path.file_stem().and_then(|s| s.to_str()) {
        let valid = !username.starts_with('-')
            && username
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if valid {
            let _ = std::process::Command::new("chown")
                .arg(format!("{username}:{username}"))
                .arg("--")
                .arg(path)
                .status();
        }
    }

    Ok(())
}
