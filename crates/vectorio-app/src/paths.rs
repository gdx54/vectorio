//! Carpeta de datos del usuario (`~/Library/Application Support/Vectorio`).

use std::path::PathBuf;

pub fn support_dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support/Vectorio"))
}
