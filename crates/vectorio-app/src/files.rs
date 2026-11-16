//! Sustitución atómica: nunca se trunca el archivo que conserva el usuario.

use std::{fs::File, io::{self, Read, Write}, path::Path};

pub fn read_project(path: &Path) -> io::Result<String> {
    let limit = vectorio_core::doc::MAX_PROJECT_BYTES;
    let mut data = String::new();
    File::open(path)?.take(limit as u64 + 1).read_to_string(&mut data)?;
    if data.len() > limit { return Err(io::Error::other("el proyecto supera el límite de 256 MB")); }
    Ok(data)
}

pub fn atomic_write(path: &Path, data: &[u8]) -> io::Result<()> {
    atomic_write_with(path, |file| file.write_all(data))
}

fn atomic_write_with(path: &Path, write: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    // Temporal único, en el mismo volumen; su Drop lo elimina ante cualquier error.
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    match std::fs::metadata(path) {
        Ok(metadata) => temp.as_file().set_permissions(metadata.permissions())?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {},
        Err(error) => return Err(error),
    }
    write(temp.as_file_mut())?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_replaces_a_project_without_touching_neighboring_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dibujo.vectorio");
        let neighbor = dir.path().join("dibujo.tmp");
        std::fs::write(&neighbor, b"otro archivo").unwrap();
        atomic_write(&path, b"proyecto largo anterior").unwrap();
        atomic_write(&path, b"nuevo").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"nuevo");
        assert_eq!(std::fs::read(&neighbor).unwrap(), b"otro archivo");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn interrupted_write_keeps_the_previous_project_and_removes_the_temporary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dibujo.vectorio");
        std::fs::write(&path, b"proyecto anterior").unwrap();
        let result = atomic_write_with(&path, |file| {
            file.write_all(b"proyecto incompleto")?;
            Err(io::Error::other("escritura interrumpida"))
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"proyecto anterior");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_rename_keeps_the_destination_and_removes_the_temporary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("carpeta");
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("conservar"), b"datos").unwrap();
        assert!(atomic_write(&path, b"nuevo").is_err());
        assert_eq!(std::fs::read(path.join("conservar")).unwrap(), b"datos");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
