//! Datos editables de una pasada. Los parámetros geométricos viven en cada variante.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Visible,
    All,
    Hidden,
}
/// Máscara de la superficie visible: intensidad difusa N·L en [0, 1].
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum TonalMask {
    #[default]
    All,
    Light,
    Shadow,
    Band {
        min: f64,
        max: f64,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DashPattern {
    pub dash_mm: f64,
    pub gap_mm: f64,
}
impl Default for DashPattern {
    fn default() -> Self {
        Self {
            dash_mm: 2.,
            gap_mm: 1.,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HatchMode {
    Parallel,
    Form,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ToneHatchParams {
    pub mode: HatchMode,
    pub levels: usize,
    pub spacing_mm: f64,
    pub angles: [f64; 4],
    pub cuts: usize,
    pub direction: [f64; 3],
}
impl Default for ToneHatchParams {
    fn default() -> Self {
        Self {
            mode: HatchMode::Parallel,
            levels: 4,
            spacing_mm: 0.8,
            angles: [0., 45., 90., 135.],
            cuts: 96,
            direction: [0., 0., 1.],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurveFamily {
    Primary,
    Secondary,
    All,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SectionParams {
    pub field: Field,
    pub direction: [f64; 3],
    pub cuts: usize,
    pub spacing: f64,
    pub offset: f64,
    pub range: [f64; 2],
    pub major_every: usize,
    pub family: CurveFamily,
    pub shift_mm: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaveParams {
    pub facets: usize,
    pub lines: usize,
    pub pattern: WavePattern,
    pub origin_azimuth: f64,
    pub origin_elevation: f64,
    pub deformation: f64,
    pub frequency: f64,
    pub major_every: usize,
    pub family: CurveFamily,
    /// El contorno comparte la familia en el estilo histórico Ondas y malla.
    pub outline: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeshParams {
    pub facets: usize,
    /// Triángulos de la red dibujada; `facets` queda para la forma y la ocultación.
    #[serde(default = "super::art::full_net")]
    pub mesh_facets: usize,
    pub coloring: MeshColor,
    pub inks: usize,
    pub group: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OutlineParams {
    Surface { facets: usize },
    Shadow(ShadowSettings),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PassKind {
    Outline(OutlineParams),
    Edges {
        angle: f64,
    },
    Sections(SectionParams),
    Field(SectionParams),
    Waves(WaveParams),
    Mesh(MeshParams),
    /// Conserva la unión de sombra propia y suelo, sin repasar sus juntas.
    Hatch {
        settings: ShadowSettings,
        middle: bool,
    },
    Ground(ShadowSettings),
    ToneHatch(ToneHatchParams),
    Reading(ReadingParams),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pass {
    pub name: String,
    pub enabled: bool,
    pub pen: usize,
    pub visibility: Visibility,
    #[serde(default)]
    pub mask: TonalMask,
    #[serde(default)]
    pub dash: DashPattern,
    #[serde(default)]
    pub reading: Option<ReadingMap>,
    #[serde(default)]
    pub finish: FinishSettings,
    pub kind: PassKind,
}
impl Pass {
    pub fn new(name: impl Into<String>, kind: PassKind, pen: usize) -> Self {
        Self {
            name: name.into(),
            kind,
            pen,
            enabled: true,
            visibility: Visibility::Visible,
            mask: Default::default(),
            dash: Default::default(),
            reading: None,
            finish: Default::default(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        self.finish.validate()?;
        if self.finish.halo_mm > 0. && !self.supports_halo() {
            return Err("El halo requiere una pasada de líneas visibles sobre la malla.".into());
        }
        if self.finish.echo_count > 0 && !self.supports_echo() {
            return Err("El eco se aplica a una pasada de contorno.".into());
        }
        if let Some(map) = &self.reading {
            map.validate()?;
            if matches!(map.encoding, Encoding::Optical | Encoding::Isolines)
                && !matches!(
                    self.kind,
                    PassKind::Reading(ReadingParams {
                        effect: ReadingEffect::Surface,
                        ..
                    })
                )
            {
                return Err(
                    "La mezcla óptica y las isolíneas requieren una pasada de mapa de superficie."
                        .into(),
                );
            }
        }
        if let PassKind::Reading(p) = &self.kind {
            p.validate()?;
            if self.visibility != Visibility::Visible {
                return Err("Las pasadas de lectura generan regiones visibles; usa la visibilidad en una pasada de líneas 3D.".into());
            }
            if self.mask != TonalMask::All
                && matches!(
                    p.effect,
                    ReadingEffect::Unfold { .. }
                        | ReadingEffect::Anaglyph { .. }
                        | ReadingEffect::Voxels { .. }
                        | ReadingEffect::Spin { .. }
                        | ReadingEffect::Section { .. }
                        | ReadingEffect::Horizons { .. }
                )
            {
                return Err("Esta vista transforma la proyección y no admite la máscara tonal de la cámara principal.".into());
            }
            let Some(map) = &self.reading else {
                return Err("La pasada de lectura necesita una medida y una rampa.".into());
            };
            if matches!(p.effect, ReadingEffect::Voxels { .. }) && (map.pens.len() != 3 || map.ramp == Ramp::Sequential) {
                return Err("El volumen en vóxeles necesita una tinta por eje X, Y y Z.".into());
            }
            let required = match p.effect {
                ReadingEffect::Anaglyph { .. } | ReadingEffect::Ridges { .. } => 2,
                ReadingEffect::Unfold { .. } | ReadingEffect::Section { .. } => 3,
                _ => 1,
            };
            if map.pens.len() < required {
                return Err(format!("Esta pasada necesita al menos {required} tintas."));
            }
        }
        if self.name.len() > 4096 {
            return Err("Nombre de pasada demasiado largo.".into());
        }
        if let TonalMask::Band { min, max } = self.mask {
            if !min.is_finite() || !max.is_finite() || min < 0. || max > 1. || min >= max {
                return Err("Banda de luz fuera de rango (0–1).".into());
            }
        }
        if !self.dash.dash_mm.is_finite()
            || !self.dash.gap_mm.is_finite()
            || !(0.1..=50.).contains(&self.dash.dash_mm)
            || !(0.1..=50.).contains(&self.dash.gap_mm)
        {
            return Err("Guion y hueco deben medir entre 0,1 y 50 mm.".into());
        }
        if let PassKind::ToneHatch(p) = &self.kind {
            if !(2..=4).contains(&p.levels)
                || !(0.08..=4.).contains(&p.spacing_mm)
                || !p.spacing_mm.is_finite()
                || !(4..=160).contains(&p.cuts)
                || p.angles.iter().chain(&p.direction).any(|v| !v.is_finite())
                || v3(p.direction).length() < 1e-9
            {
                return Err("Parámetros de trama tonal fuera de rango.".into());
            }
        }
        let (settings, _) = super::passes::program(&Settings::default(), self);
        settings.validate()?;
        if let PassKind::Mesh(params) = &self.kind {
            // Hasta tres familias vacías se conservan para adaptar la interfaz antigua.
            if params.group >= params.inks.max(3) {
                return Err("Grupo de malla fuera de rango.".into());
            }
        }
        Ok(())
    }
}
