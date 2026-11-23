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

/// Nombre de archivo legible derivado del proyecto: sin separadores ni caracteres reservados.
pub fn file_stem(name: &str, fallback: &str) -> String {
    let clean: String = name.chars().filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control()).collect();
    let clean = clean.trim().trim_start_matches('.').chars().take(80).collect::<String>();
    if clean.is_empty() { fallback.to_owned() } else { clean }
}

/// Escribe un lote en una carpeta nueva dentro de `parent` («nombre», «nombre 2»…). Nunca
/// sustituye archivos existentes; si una escritura falla se retira la carpeta entera, de modo
/// que no queda un lote a medias.
pub fn write_batch(parent: &Path, folder: &str, files: &[(String, Vec<u8>)]) -> io::Result<std::path::PathBuf> {
    let mut n = 1;
    let dir = loop {
        let name = if n == 1 { folder.to_owned() } else { format!("{folder} {n}") };
        let candidate = parent.join(name);
        match std::fs::create_dir(&candidate) {
            Ok(()) => break candidate,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists && n < 999 => n += 1,
            Err(error) => return Err(error),
        }
    };
    for (name, data) in files {
        let result = File::create_new(dir.join(name)).and_then(|mut f| { f.write_all(data)?; f.sync_all() });
        if let Err(error) = result {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(error);
        }
    }
    Ok(dir)
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

    #[test]
    fn batches_go_to_a_new_folder_and_never_replace_previous_exports() {
        let dir = tempfile::tempdir().unwrap();
        let files = vec![("a.svg".to_owned(), b"uno".to_vec()), ("b.svg".to_owned(), b"dos".to_vec())];
        let first = write_batch(dir.path(), "Retrato", &files).unwrap();
        std::fs::write(first.join("a.svg"), b"retocado").unwrap();
        let second = write_batch(dir.path(), "Retrato", &files).unwrap();
        assert_eq!(first.file_name().unwrap(), "Retrato");
        assert_eq!(second.file_name().unwrap(), "Retrato 2");
        assert_eq!(std::fs::read(first.join("a.svg")).unwrap(), b"retocado");
        assert_eq!(std::fs::read(second.join("b.svg")).unwrap(), b"dos");
    }

    #[test]
    fn a_failed_batch_leaves_no_partial_folder() {
        let dir = tempfile::tempdir().unwrap();
        let files = vec![("a.svg".to_owned(), b"uno".to_vec()), ("a.svg".to_owned(), b"repetido".to_vec())];
        assert!(write_batch(dir.path(), "Lote", &files).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn suggested_names_come_from_the_project_without_path_separators() {
        assert_eq!(file_stem("Mapa: norte/sur", "Sin título"), "Mapa nortesur");
        assert_eq!(file_stem("  ", "Sin título"), "Sin título");
        assert_eq!(file_stem(".oculto", "x"), "oculto");
    }
}
