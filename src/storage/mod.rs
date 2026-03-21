pub mod read;
pub mod write;

use std::path::PathBuf;

use crate::error::RustfaceError;
use crate::face::embed::Embedding;

pub const FACES_DIR: &str = "/etc/rustface/faces";
pub const MODELS_DIR: &str = "/etc/rustface/models";

pub struct FaceStore {
    base_path: PathBuf,
}

impl FaceStore {
    pub fn new(base_path: &str) -> Self {
        Self {
            base_path: PathBuf::from(base_path),
        }
    }

    pub fn default() -> Self {
        Self::new(FACES_DIR)
    }

    fn user_path(&self, username: &str) -> PathBuf {
        self.base_path.join(format!("{username}.bin"))
    }

    pub fn exists(&self, username: &str) -> bool {
        self.user_path(username).exists()
    }

    pub fn load(&self, username: &str) -> Result<Embedding, RustfaceError> {
        let path = self.user_path(username);
        if !path.exists() {
            return Err(RustfaceError::NoEnrolledFace {
                username: username.to_owned(),
            });
        }
        read::read_embedding(&path)
    }

    pub fn save(&self, username: &str, embedding: &Embedding) -> Result<(), RustfaceError> {
        let path = self.user_path(username);
        write::write_embedding(&path, embedding)
    }
}
