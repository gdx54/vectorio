//! Biblioteca de tintas independiente de documentos y estilos de dibujo.
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
use vectorio_core::{Document, Pen};

pub const MAX_INKS: usize = 64;
const MAX_PALETTES: usize = 128;
const MAX_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Palette {
    pub name: String,
    pub pens: Vec<Pen>,
}

impl Palette {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.name.trim().is_empty() || self.name.chars().count() > 256 {
            return Err(tm!("Escribe un nombre de paleta de hasta 256 caracteres."));
        }
        if self.pens.is_empty() || self.pens.len() > MAX_INKS {
            return Err(tm!("Una paleta debe tener entre 1 y 64 tintas."));
        }
        if self.pens.iter().any(|p| {
            p.name.trim().is_empty()
                || p.name.chars().count() > 256
                || !p.width_mm.is_finite()
                || p.width_mm <= 0.
                || p.width_mm > 100.
        }) {
            return Err(tm!(
                "Cada tinta necesita un nombre y un grosor entre 0 y 100 mm."
            ));
        }
        Ok(())
    }

    /// Solo añade tintas que no existen: nunca cambia índices ni colores anteriores.
    pub fn apply(&self, doc: &mut Document) -> Vec<usize> {
        self.pens
            .iter()
            .map(|pen| {
                if let Some(i) = doc.pens.iter().position(|p| same_ink(p, pen)) {
                    i
                } else {
                    let i = doc.pens.len();
                    doc.pens.push(pen.clone());
                    i
                }
            })
            .collect()
    }

    pub fn matches(&self, indices: &[usize], pens: &[Pen]) -> bool {
        indices.len() == self.pens.len()
            && indices
                .iter()
                .zip(&self.pens)
                .all(|(i, pen)| pens.get(*i).is_some_and(|p| same_ink(p, pen)))
    }
}

fn same_ink(a: &Pen, b: &Pen) -> bool {
    a.color == b.color && a.width_mm == b.width_mm
}

pub fn presets() -> Vec<Palette> {
    vectorio_core::volumetric::InkSet::ALL
        .into_iter()
        .map(|s| Palette {
            name: s.name().into(),
            pens: s.pens(),
        })
        .collect()
}

#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    palettes: Vec<Palette>,
}

fn validate(palettes: &[Palette]) -> Result<(), &'static str> {
    if palettes.len() > MAX_PALETTES {
        return Err(tm!("La biblioteca admite hasta 128 paletas."));
    }
    for (i, palette) in palettes.iter().enumerate() {
        palette.validate()?;
        if palettes[..i]
            .iter()
            .any(|p| p.name.trim().to_lowercase() == palette.name.trim().to_lowercase())
        {
            return Err(tm!("Ya existe una paleta con ese nombre. Elige otro."));
        }
    }
    Ok(())
}

fn read(path: &Path) -> Result<Vec<Palette>, &'static str> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(_) => {
            return Err(tm!(
                "No se pudo leer la biblioteca de paletas. Se conserva el archivo."
            ))
        }
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| tm!("No se pudo leer la biblioteca de paletas. Se conserva el archivo."))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(tm!(
            "La biblioteca de paletas supera 1 MB. Se conserva el archivo."
        ));
    }
    let file: File = serde_json::from_slice(&bytes)
        .map_err(|_| tm!("La biblioteca de paletas no es válida. Se conserva el archivo."))?;
    if file.version != 1 {
        return Err(tm!(
            "Esta versión de la biblioteca de paletas no es compatible."
        ));
    }
    validate(&file.palettes)?;
    Ok(file.palettes)
}

#[derive(Default)]
pub struct Library {
    pub saved: Vec<Palette>,
    path: Option<PathBuf>,
    pub error: Option<&'static str>,
}

impl Library {
    pub fn load_user() -> Self {
        match crate::paths::support_dir() {
            Some(dir) => Self::at(dir.join("paletas.json")),
            None => Self {
                error: Some(tm!("No se encuentra la carpeta de usuario.")),
                ..Self::default()
            },
        }
    }

    fn at(path: PathBuf) -> Self {
        let mut library = Self {
            path: Some(path),
            ..Self::default()
        };
        let _ = library.reload();
        library
    }

    pub fn reload(&mut self) -> Result<(), &'static str> {
        if let Some(path) = &self.path {
            match read(path) {
                Ok(saved) => {
                    self.saved = saved;
                    self.error = None;
                }
                Err(e) => {
                    self.error = Some(e);
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    fn commit(&mut self, palettes: Vec<Palette>) -> Result<(), &'static str> {
        validate(&palettes)?;
        // Una biblioteca ilegible no se reemplaza con una lista vacía.
        if let Some(error) = self.error {
            return Err(error);
        }
        if let Some(path) = &self.path {
            if read(path)? != self.saved {
                return Err(tm!(
                    "La biblioteca cambió en otra ventana. Pulsa Recargar antes de guardar."
                ));
            }
            let bytes = serde_json::to_vec_pretty(&File {
                version: 1,
                palettes: palettes.clone(),
            })
            .map_err(|_| tm!("No se pudo guardar la paleta. Tus cambios siguen en el editor."))?;
            if bytes.len() as u64 > MAX_BYTES {
                return Err(tm!(
                    "La biblioteca de paletas supera 1 MB. Se conserva el archivo."
                ));
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|_| {
                    tm!("No se pudo guardar la paleta. Tus cambios siguen en el editor.")
                })?;
            }
            crate::files::atomic_write(path, &bytes).map_err(|_| {
                tm!("No se pudo guardar la paleta. Tus cambios siguen en el editor.")
            })?;
        }
        self.saved = palettes;
        Ok(())
    }

    /// `None` crea una entrada; `Some` edita una existente sin sobrescribir homónimos.
    pub fn store(
        &mut self,
        index: Option<usize>,
        mut palette: Palette,
    ) -> Result<usize, &'static str> {
        palette.name = palette.name.trim().to_owned();
        for pen in &mut palette.pens {
            pen.name = pen.name.trim().to_owned();
        }
        let mut next = self.saved.clone();
        let i = index.unwrap_or(next.len());
        if index.is_some() && i >= next.len() {
            return Err(tm!("La paleta ya no existe. Pulsa Recargar."));
        }
        if index.is_some() {
            next[i] = palette;
        } else {
            next.push(palette);
        }
        self.commit(next)?;
        Ok(i)
    }

    pub fn remove(&mut self, index: usize) -> Result<(), &'static str> {
        if index >= self.saved.len() {
            return Err(tm!("La paleta ya no existe. Pulsa Recargar."));
        }
        let mut next = self.saved.clone();
        next.remove(index);
        self.commit(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_roundtrip_rename_and_delete_keep_order_names_colors_and_widths() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("paletas.json");
        let mut library = Library::at(path.clone());
        let mut palette = presets().remove(1);
        palette.name = " Mi paleta ".into();
        palette.pens[0].custom_name = true;
        palette.pens[0].name = "Mi cian".into();
        palette.pens[0].width_mm = 0.73;
        library.store(None, palette.clone()).unwrap();
        palette.name = "Mi paleta".into();
        assert_eq!(Library::at(path.clone()).saved, vec![palette.clone()]);
        assert!(library
            .store(
                None,
                Palette {
                    name: "mi paleta".into(),
                    ..palette.clone()
                }
            )
            .is_err());
        palette.name = "Otra paleta".into();
        palette.pens.swap(0, 2);
        library.store(Some(0), palette.clone()).unwrap();
        assert_eq!(Library::at(path.clone()).saved, vec![palette]);
        library.remove(0).unwrap();
        assert!(Library::at(path).saved.is_empty());
    }

    #[test]
    fn corrupt_unsupported_and_stale_files_are_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("paletas.json");
        for bytes in [
            b"broken".as_slice(),
            br#"{"version":9,"palettes":[]}"#.as_slice(),
        ] {
            std::fs::write(&path, bytes).unwrap();
            let mut library = Library::at(path.clone());
            assert!(library.store(None, presets().remove(0)).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        std::fs::remove_file(&path).unwrap();
        let mut first = Library::at(path.clone());
        let mut second = Library::at(path.clone());
        first.store(None, presets().remove(0)).unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(second.store(None, presets().remove(1)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        second.reload().unwrap();
        second.store(None, presets().remove(1)).unwrap();
        assert_eq!(Library::at(path).saved.len(), 2);
    }

    #[test]
    fn application_reuses_physical_inks_and_never_recolors_existing_objects() {
        let mut doc = Document::default();
        doc.add(vectorio_core::Item::new(
            "Prueba",
            vectorio_core::Geometry::Path {
                lines: vec![vec![
                    vectorio_core::P { x: 0., y: 0. },
                    vectorio_core::P { x: 5., y: 5. },
                ]],
                closed: false,
            },
            vectorio_core::P { x: 10., y: 10. },
        ));
        let original = doc.clone();
        let palette = presets().remove(1);
        let indices = palette.apply(&mut doc);
        assert_eq!(doc.items, original.items);
        assert_eq!(&doc.pens[..original.pens.len()], original.pens);
        assert!(palette.matches(&indices, &doc.pens));
        let count = doc.pens.len();
        assert_eq!(palette.apply(&mut doc), indices);
        assert_eq!(doc.pens.len(), count);
        let mut changed = palette.clone();
        changed.pens[0].width_mm *= 2.;
        assert_ne!(changed.apply(&mut doc)[0], indices[0]);
    }

    #[test]
    fn invalid_ink_widths_and_unbounded_palettes_are_rejected() {
        let mut palette = presets().remove(0);
        for width in [0., -1., f64::NAN, f64::INFINITY, 101.] {
            palette.pens[0].width_mm = width;
            assert!(palette.validate().is_err());
        }
        palette.pens = vec![Pen::new("Tinta", [0, 0, 0], 0.4); MAX_INKS + 1];
        assert!(palette.validate().is_err());
    }
}
