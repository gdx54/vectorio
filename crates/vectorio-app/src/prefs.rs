//! Preferencias de la aplicación, guardadas en `preferencias.json` dentro de la carpeta de
//! datos. Valen para todos los proyectos; cada `.vectorio` conserva sus propios ajustes.

use crate::theme::Appearance;
use serde::{Deserialize, Serialize};
use std::path::Path;
use vectorio_core::{output::OutputSettings, Document, Paper, PlotSettings};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "sistema")]
    System,
    #[serde(rename = "es")]
    Spanish,
    #[serde(rename = "en")]
    English,
}

impl Language {
    pub const ALL: [Self; 3] = [Self::System, Self::Spanish, Self::English];
    pub fn label(self) -> String { match self { Self::System => tr!("Sistema"), Self::Spanish => "Español".into(), Self::English => "English".into() } }
    /// Código de la lengua que se usa: «es» o «en». «Sistema» elige español si el Mac está en español.
    pub fn resolve(self) -> &'static str {
        match self {
            Self::Spanish => "es",
            Self::English => "en",
            Self::System => if sys_locale::get_locale().is_some_and(|l| l.to_lowercase().starts_with("es")) { "es" } else { "en" },
        }
    }
}

/// Valores con los que empieza cada documento nuevo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NewDocument {
    pub paper: Paper,
    pub plot: PlotSettings,
    pub output: OutputSettings,
}

impl Default for NewDocument {
    fn default() -> Self {
        let doc = Document::default();
        Self { paper: doc.paper, plot: doc.plot, output: doc.output }
    }
}

impl NewDocument {
    /// Documento nuevo. Las tintas de serie llevan su nombre en español y se muestran traducidas.
    pub fn document(&self) -> Document {
        Document { paper: self.paper.clone(), plot: self.plot.clone(), output: self.output.clone(), ..Default::default() }
    }
    pub fn from_document(doc: &Document) -> Self {
        Self { paper: doc.paper.clone(), plot: doc.plot.clone(), output: doc.output.clone() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub appearance: Appearance,
    pub language: Language,
    /// Factor de tamaño de toda la interfaz (0,8 a 1,5).
    pub ui_scale: f32,
    /// Color del papel en la vista previa (índice de `PAPER_TINTS`).
    pub paper_tint: usize,
    pub plotter_name: String,
    pub show_tips: bool,
    pub new_document: NewDocument,
    pub create: crate::create::CreatePrefs,
}

pub const SCALE_RANGE: std::ops::RangeInclusive<f32> = 0.8..=1.5;

impl Default for Preferences {
    fn default() -> Self {
        Self { appearance: Appearance::System, language: Language::System, ui_scale: 1.0, paper_tint: 0,
            plotter_name: "EasyDraw".into(), show_tips: true, new_document: NewDocument::default(), create: Default::default() }
    }
}

const FILE: &str = "preferencias.json";
const OLD_APPEARANCE_FILE: &str = "apariencia.json";

impl Preferences {
    /// Valores que la interfaz puede aplicar sin fallar, aunque el archivo trajera basura.
    pub fn sanitized(mut self) -> Self {
        self.ui_scale = if self.ui_scale.is_finite() { self.ui_scale.clamp(*SCALE_RANGE.start(), *SCALE_RANGE.end()) } else { 1.0 };
        self.paper_tint = self.paper_tint.min(crate::app::PAPER_TINTS.len() - 1);
        self
    }

    /// Nombre del plotter para mostrar; si se deja en blanco, el de siempre.
    pub fn plotter(&self) -> &str {
        if self.plotter_name.trim().is_empty() { "EasyDraw" } else { self.plotter_name.trim() }
    }

    pub fn load() -> Self {
        crate::paths::support_dir().map(|dir| Self::load_from(&dir)).unwrap_or_default()
    }
    pub fn save(&self) -> Result<(), String> {
        let dir = crate::paths::support_dir().ok_or("No se encuentra la carpeta de usuario")?;
        self.save_to(&dir)
    }

    /// Lee las preferencias de `dir`. Si aún no existen pero sí el antiguo `apariencia.json`,
    /// toma el aspecto de ahí y crea el archivo nuevo sin borrar el viejo.
    pub fn load_from(dir: &Path) -> Self {
        if let Some(prefs) = std::fs::read(dir.join(FILE)).ok().and_then(|b| serde_json::from_slice::<Self>(&b).ok()) {
            return prefs.sanitized();
        }
        let old = std::fs::read(dir.join(OLD_APPEARANCE_FILE)).ok().and_then(|b| serde_json::from_slice::<Appearance>(&b).ok());
        let prefs = Self { appearance: old.unwrap_or_default(), ..Self::default() };
        if old.is_some() { let _ = prefs.save_to(dir); }
        prefs
    }

    pub fn save_to(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let file = dir.join(FILE);
        let temp = file.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        std::fs::rename(temp, file).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vectorio-prefs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn round_trips_and_missing_fields_use_defaults() {
        let prefs = Preferences { appearance: Appearance::Dark, ui_scale: 1.2, paper_tint: 2, plotter_name: "AxiDraw".into(), show_tips: false, ..Default::default() };
        let back: Preferences = serde_json::from_slice(&serde_json::to_vec(&prefs).unwrap()).unwrap();
        assert_eq!(back, prefs);
        assert_eq!(serde_json::from_str::<Preferences>("{}").unwrap(), Preferences::default());
        let partial: Preferences = serde_json::from_str(r#"{"appearance":"oscuro"}"#).unwrap();
        assert_eq!((partial.appearance, partial.ui_scale, partial.plotter_name.as_str()), (Appearance::Dark, 1.0, "EasyDraw"));
    }

    #[test]
    fn saves_atomically_and_loads_back() {
        let dir = temp_dir("guardar");
        let prefs = Preferences { ui_scale: 1.3, ..Default::default() };
        prefs.save_to(&dir).unwrap();
        assert!(!dir.join("preferencias.json.tmp").exists());
        assert_eq!(Preferences::load_from(&dir), prefs);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn migrates_the_old_appearance_file_without_deleting_it() {
        let dir = temp_dir("migrar");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("apariencia.json"), br#""oscuro""#).unwrap();
        let prefs = Preferences::load_from(&dir);
        assert_eq!(prefs.appearance, Appearance::Dark);
        assert!(dir.join("apariencia.json").exists() && dir.join("preferencias.json").exists());
        assert_eq!(Preferences::load_from(&dir).appearance, Appearance::Dark);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn broken_files_and_wild_values_fall_back_safely() {
        let dir = temp_dir("roto");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("preferencias.json"), b"no es json").unwrap();
        assert_eq!(Preferences::load_from(&dir), Preferences::default());
        std::fs::write(dir.join("preferencias.json"), br#"{"ui_scale": 40.0, "paper_tint": 99, "plotter_name": "  "}"#).unwrap();
        let prefs = Preferences::load_from(&dir);
        assert_eq!((prefs.ui_scale, prefs.paper_tint, prefs.plotter()), (1.5, 4, "EasyDraw"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn new_documents_start_from_the_chosen_defaults() {
        let mut nd = NewDocument::default();
        assert_eq!(nd.document(), Document::default());
        nd.paper = vectorio_core::Paper::presets().remove(3);
        nd.output.dry_seconds = 45.0;
        let doc = nd.document();
        assert_eq!((doc.paper.name.as_str(), doc.output.dry_seconds), ("A4 vertical", 45.0));
        assert_eq!(NewDocument::from_document(&doc), nd);
    }
}
