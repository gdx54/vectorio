//! VOLUMETRIC: modelo → campo → curvas de superficie → proyección → dibujo físico.
//! No contiene estado de interfaz ni colores. La cámara y oclusión son de `scene`.
mod art;
mod camera;
pub use camera::Camera;
mod continuity;
#[cfg(test)]
mod continuity_tests;
mod contours;
pub use continuity::{ProjectionSettings, SheetViews, VolumeBinding, VolumeObject};
mod engine;
mod finishing;
mod import;
mod mesh;
mod pass;
mod passes;
#[cfg(test)]
mod phase_d_tests;
mod pipeline;
mod reading;
mod reduce;
mod shadow;
mod stl;
mod style;
#[cfg(test)]
mod tests;
pub use crate::scene::V3;
pub use art::{ArtDrawing, ArtSettings, MeshColor, MeshStroke, WavePattern};
pub use contours::{field_values, isolines, Contour, Field};
pub use engine::{Drawing, DrawingPart, Engine, InkFamily};
pub use finishing::FinishSettings;
pub use import::{load_obj, parse_obj};
pub use mesh::{
    Mesh, MeshReport, Primitive, MAX_FACES, MAX_FILE_BYTES, MAX_RECIPE_BYTES, MAX_VERTICES,
};
pub use pass::{
    CurveFamily, DashPattern, HatchMode, MeshParams, OutlineParams, Pass, PassKind, SectionParams,
    TonalMask, ToneHatchParams, Visibility, WaveParams,
};
pub use pipeline::feature_edges;
pub use reading::{
    lightness, reading_content_paper, ColorMap, Encoding, InkSet, Measure, OpticalInk,
    OpticalSettings, Ramp, ReadingEffect, ReadingMap, ReadingParams,
};
pub use reduce::MeshDetail;
use serde::{Deserialize, Serialize};
pub use shadow::{ShadedFace, ShadowDrawing, ShadowSettings};
pub use stl::{load_mesh, load_mesh_cancellable, load_stl, parse_stl};
pub use style::{ArtisticStyle, ReadingStyle, TonalStyle};
pub(super) fn v3(q: [f64; 3]) -> V3 {
    V3::new(q[0], q[1], q[2])
}
pub(super) fn arr(q: V3) -> [f64; 3] {
    [q.x, q.y, q.z]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecipeKind {
    Atlas,
    Strata,
    XRay,
    FlatShadow,
    SurfaceWaves,
    TriMesh,
    Hybrid,
}
impl RecipeKind {
    pub fn is_art(self) -> bool {
        matches!(self, Self::SurfaceWaves | Self::TriMesh | Self::Hybrid)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub recipe: RecipeKind,
    pub projection: ProjectionSettings,
    pub sheet: SheetViews,
    pub view_count: usize,
    pub view_gap_mm: f64,
    pub mesh_detail: MeshDetail,
    pub shadow: ShadowSettings,
    pub art: ArtSettings,
    pub field: Field,
    pub direction: [f64; 3],
    pub cuts: usize,
    /// Si > 0, distancia en unidades del modelo normalizado.
    pub spacing: f64,
    pub offset: f64,
    pub range: [f64; 2],
    pub yaw: f64,
    pub elevation: f64,
    pub model_rotation: [f64; 3],
    pub model_scale: [f64; 3],
    pub visible_only: bool,
    /// Conserva los intervalos ocultos para las pasadas discontinuas.
    pub hidden_lines: bool,
    pub halo_mm: f64,
    pub major_every: usize,
    pub simplify_mm: f64,
    pub zoom: f64,
    pub position_mm: [f64; 2],
    pub paper_rotation: f64,
    pub strata_shift_mm: f64,
    pub seed: u64,
    pub edge_angle: f64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            recipe: RecipeKind::Atlas,
            projection: Default::default(),
            sheet: Default::default(),
            view_count: 6,
            view_gap_mm: 4.,
            mesh_detail: MeshDetail::Interactive,
            shadow: ShadowSettings::default(),
            art: ArtSettings::default(),
            field: Field::Height,
            direction: [0., 0., 1.],
            cuts: 48,
            spacing: 0.,
            offset: 0.,
            range: [0., 1.],
            yaw: 45.,
            elevation: 35.26438968,
            model_rotation: [0.; 3],
            model_scale: [1.; 3],
            visible_only: true,
            hidden_lines: false,
            halo_mm: 0.,
            major_every: 5,
            simplify_mm: 0.025,
            zoom: 0.88,
            position_mm: [0.; 2],
            paper_rotation: 0.,
            strata_shift_mm: 0.35,
            seed: 1,
            edge_angle: 25.,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !self.projection.distance.is_finite()
            || !(0.6..=20.).contains(&self.projection.distance)
            || !self.projection.near.is_finite()
            || !(0.01..=1.).contains(&self.projection.near)
            || !(2..=12).contains(&self.view_count)
            || !self.view_gap_mm.is_finite()
            || !(0.0..=30.).contains(&self.view_gap_mm)
        {
            return Err("Cámara u hoja multivista fuera de rango.".into());
        }
        self.shadow.validate()?;
        self.art.validate()?;
        let nums = [
            self.spacing,
            self.halo_mm,
            self.offset,
            self.yaw,
            self.elevation,
            self.simplify_mm,
            self.zoom,
            self.paper_rotation,
            self.strata_shift_mm,
            self.edge_angle,
        ];
        if nums
            .iter()
            .chain(&self.direction)
            .chain(&self.model_rotation)
            .chain(&self.model_scale)
            .chain(&self.position_mm)
            .chain(&self.range)
            .any(|x| !x.is_finite())
            || !(0.0..=5.).contains(&self.halo_mm)
            || !(1..=160).contains(&self.cuts)
            || self.major_every < 1
            || self.major_every > 160
            || self.elevation < 0.01
            || self.elevation > 89.99
            || self.zoom < 0.05
            || self.zoom > 3.
            || self.spacing < 0.
            || self.spacing > 10.
            || self.offset.abs() > 10.
            || self.simplify_mm < 0.
            || self.simplify_mm > 1.
            || self.strata_shift_mm.abs() > 5.
            || self.range[0] < 0.
            || self.range[1] > 1.
            || self.range[0] >= self.range[1]
            || self.model_scale.iter().any(|x| *x < 0.05 || *x > 10.)
            || self.position_mm.iter().any(|x| x.abs() > 1000.)
            || v3(self.direction).length() < 1e-9
            || v3(self.direction).length() > 1e6
            || !(0.0..=180.).contains(&self.edge_angle)
        {
            return Err("Parámetros VOLUMETRIC fuera de rango.".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub version: u32,
    pub mesh: Mesh,
    pub settings: Settings,
    #[serde(default)]
    pub paper: crate::Paper,
    #[serde(default)]
    pub inks: Vec<crate::Pen>,
    /// Pila autoritativa de la receta v2.
    pub passes: Vec<Pass>,
}
impl Recipe {
    pub fn new(mesh: Mesh, settings: Settings) -> Self {
        let mut passes = settings.passes();
        for pass in &mut passes {
            pass.pen = 0;
        }
        Self {
            version: 2,
            mesh,
            settings,
            passes,
            paper: Default::default(),
            inks: vec![],
        }
    }
    pub fn from_json(text: &str) -> Result<Self, String> {
        if text.len() as u64 > MAX_RECIPE_BYTES {
            return Err("Receta demasiado grande (máximo 512 MB).".into());
        }
        let mut value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        match value.get("version").and_then(|v| v.as_u64()) {
            Some(1) => {
                let settings: Settings = serde_json::from_value(
                    value
                        .get("settings")
                        .cloned()
                        .ok_or("Faltan los parámetros de receta.")?,
                )
                .map_err(|e| e.to_string())?;
                let pens: [usize; 3] = match value.get("layer_pens") {
                    Some(pens) => {
                        serde_json::from_value(pens.clone()).map_err(|e| e.to_string())?
                    }
                    None => [0; 3],
                };
                let mut passes = settings.passes();
                for pass in &mut passes {
                    pass.pen = pens.get(pass.pen).copied().unwrap_or(0);
                }
                value["passes"] = serde_json::to_value(passes).map_err(|e| e.to_string())?;
                value["version"] = 2.into();
            }
            Some(2) => {}
            _ => return Err("Versión de receta VOLUMETRIC no compatible.".into()),
        }
        let mut r: Self = serde_json::from_value(value).map_err(|e| e.to_string())?;
        for pass in &r.passes {
            pass.validate()?;
        }
        r.settings.validate()?;
        if r.inks.iter().any(|p| {
            p.name.len() > 4096 || !p.width_mm.is_finite() || !(0.01..=10.).contains(&p.width_mm)
        }) || (!r.inks.is_empty()
            && r.passes.iter().any(|pass| {
                pass.pen >= r.inks.len()
                    || pass
                        .reading
                        .as_ref()
                        .is_some_and(|m| m.pens.iter().any(|i| *i >= r.inks.len()))
            }))
        {
            return Err("Tintas de receta no válidas.".into());
        }
        if ![r.paper.width_mm, r.paper.height_mm, r.paper.margin_mm]
            .iter()
            .all(|x| x.is_finite())
            || r.paper.width_mm < 10.
            || r.paper.height_mm < 10.
            || r.paper.width_mm > 2000.
            || r.paper.height_mm > 2000.
            || r.paper.margin_mm < 0.
            || 2. * r.paper.margin_mm >= r.paper.width_mm.min(r.paper.height_mm)
        {
            return Err("Papel de receta no válido.".into());
        }
        r.mesh = r.mesh.validated()?.0;
        Ok(r)
    }
    /// Asigna las tintas de los papeles históricos a las pasadas creadas por el estilo.
    pub fn set_layer_pens(&mut self, pens: impl AsRef<[usize]>) {
        for (i, pass) in self.passes.iter_mut().enumerate() {
            pass.pen = pens.as_ref().get(i).copied().unwrap_or(pass.pen);
        }
    }
    pub fn to_json(&self) -> Result<String, String> {
        let mut recipe = self.clone();
        recipe.version = 2;
        let text = serde_json::to_string(&recipe).map_err(|e| e.to_string())?;
        if text.len() as u64 > MAX_RECIPE_BYTES {
            return Err("La receta supera 512 MB; conserva también el modelo de origen.".into());
        }
        Ok(text)
    }
}
