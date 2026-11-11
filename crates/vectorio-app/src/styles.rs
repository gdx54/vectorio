//! Estilos de trazo: unos predefinidos y los que guarda el usuario, que se conservan
//! entre sesiones en `~/Library/Application Support/Vectorio/estilos.json`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use vectorio_core::{TraceParams, TraceStyle, Vibe};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedStyle {
    pub name: String,
    pub trace: TraceParams,
}

/// Estilos que trae Vectorio.
pub fn presets() -> Vec<SavedStyle> {
    let base = TraceParams::default();
    let style = |name: &str, trace: TraceParams| SavedStyle { name: name.into(), trace };
    vec![
        style("Grabado", TraceParams { spacing_mm: 0.7, dash_mm: 30.0, weight: 0.5, ..base.clone() }),
        style("Curvas de nivel", TraceParams { style: TraceStyle::Contour, spacing_mm: 1.0, dash_mm: 25.0, ..base.clone() }),
        style("Corrientes", TraceParams { style: TraceStyle::Flow, spacing_mm: 0.9, dash_mm: 12.0, dash_vary: 0.6, ..base.clone() }),
        style("Nervioso", TraceParams { style: TraceStyle::Flow, dash_mm: 8.0, vibe: Vibe::Tremor, vibe_mm: 0.6, wave_mm: 3.0, ..base.clone() }),
        style("Ondas", TraceParams { spacing_mm: 1.2, dash_mm: 40.0, vibe: Vibe::Wave, vibe_mm: 0.5, wave_mm: 2.0, ..base.clone() }),
        style("Espiral", TraceParams { style: TraceStyle::Spiral, spacing_mm: 1.0, dash_mm: 6.0, ..base.clone() }),
        style("Garabato", TraceParams { style: TraceStyle::Scribble, spacing_mm: 0.9, ..base.clone() }),
        style("Puntillismo", TraceParams { style: TraceStyle::Dots, spacing_mm: 1.2, weight: 0.6, ..base }),
    ]
}

fn file() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/Vectorio/estilos.json"))
}

/// Estilos guardados por el usuario (vacío si no hay o el archivo no se entiende).
pub fn load() -> Vec<SavedStyle> {
    file().and_then(|f| std::fs::read_to_string(f).ok()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save(styles: &[SavedStyle]) -> Result<(), String> {
    let path = file().ok_or("No se encuentra la carpeta de usuario")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(styles).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

/// Añade o sustituye (mismo nombre) un estilo del usuario.
pub fn upsert(styles: &mut Vec<SavedStyle>, name: &str, trace: &TraceParams) {
    let name = name.trim();
    let trace = TraceParams { angle_deg: TraceParams::default().angle_deg, phase: 0.0, ..trace.clone() };
    match styles.iter_mut().find(|s| s.name.eq_ignore_ascii_case(name)) {
        Some(s) => s.trace = trace,
        None => styles.push(SavedStyle { name: name.into(), trace }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saving_a_style_twice_with_the_same_name_replaces_it() {
        let mut styles = Vec::new();
        upsert(&mut styles, " Mi rayado ", &TraceParams { spacing_mm: 2.0, angle_deg: 77.0, phase: 0.5, ..Default::default() });
        upsert(&mut styles, "mi rayado", &TraceParams { spacing_mm: 3.0, ..Default::default() });
        assert_eq!(styles.len(), 1);
        assert_eq!((styles[0].name.as_str(), styles[0].trace.spacing_mm, styles[0].trace.phase), ("Mi rayado", 3.0, 0.0));
        let json = serde_json::to_string(&styles).unwrap();
        assert_eq!(serde_json::from_str::<Vec<SavedStyle>>(&json).unwrap(), styles);
        assert!(presets().iter().all(|p| !p.name.is_empty()));
    }
}
