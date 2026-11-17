//! Medida → codificación → rampa. Campos físicos de la malla y salida vectorial.
mod effects;
mod ink_sets;
mod optical;
mod scalar;
pub use ink_sets::InkSet;
pub use optical::{ColorMap, OpticalInk, OpticalSettings};
mod encode;
#[cfg(test)]
mod extension_tests;
mod features;
mod fields;
mod legend;
mod rays;
mod section;
mod surface;
#[cfg(test)]
mod tests;
mod unfold;
mod voxels;
use super::*;
use crate::{p, BBox, Polyline, P};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Measure {
    Height,
    Depth,
    Geodesic,
    Light,
    MeanCurvature,
    GaussianCurvature,
    Slope,
    Aspect,
    AmbientOcclusion,
    Thickness,
    Objects,
}
impl Measure {
    pub const ALL: [Self; 11] = [
        Self::Height,
        Self::Depth,
        Self::Geodesic,
        Self::Light,
        Self::MeanCurvature,
        Self::GaussianCurvature,
        Self::Slope,
        Self::Aspect,
        Self::AmbientOcclusion,
        Self::Thickness,
        Self::Objects,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Height => "Altura",
            Self::Depth => "Profundidad",
            Self::Geodesic => "Distancia geodésica",
            Self::Light => "Luz",
            Self::MeanCurvature => "Curvatura media",
            Self::GaussianCurvature => "Curvatura gaussiana",
            Self::Slope => "Pendiente",
            Self::Aspect => "Orientación de pendiente",
            Self::AmbientOcclusion => "Oclusión ambiental",
            Self::Thickness => "Espesor",
            Self::Objects => "Objetos",
        }
    }
    pub fn units(self) -> &'static str {
        match self {
            Self::Height | Self::Depth | Self::Geodesic | Self::Thickness => "u",
            Self::MeanCurvature => "1/u",
            Self::GaussianCurvature => "1/u2",
            Self::Slope | Self::Aspect => "grados",
            Self::Light | Self::AmbientOcclusion => "0-1",
            Self::Objects => "id",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Encoding {
    Bands,
    Dither,
    Density,
    Optical,
    Isolines,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ramp {
    Sequential,
    Diverging,
    Categorical,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReadingMap {
    pub measure: Measure,
    pub secondary: Option<Measure>,
    pub weights: [f64; 2],
    pub encoding: Encoding,
    pub optical: OpticalSettings,
    pub levels: usize,
    pub label_values: bool,
    pub ramp: Ramp,
    pub pens: Vec<usize>,
    pub invert: bool,
    pub equalize: bool,
    /// None = rango de datos; Some = rango en las unidades de la medida.
    pub range: Option<[f64; 2]>,
    pub threshold: Option<f64>,
    pub samples: usize,
    pub spacing_mm: f64,
    pub angle: f64,
    pub legend: bool,
    /// Fecha explícita: se conserva al guardar y no cambia el SVG al abrir la receta.
    pub date: String,
}
impl Default for ReadingMap {
    fn default() -> Self {
        Self {
            measure: Measure::MeanCurvature,
            secondary: None,
            weights: [1., 1.],
            encoding: Encoding::Bands,
            optical: Default::default(),
            levels: 16,
            label_values: false,
            ramp: Ramp::Diverging,
            pens: vec![2, 1],
            invert: false,
            equalize: false,
            range: None,
            threshold: None,
            samples: 24,
            spacing_mm: 1.,
            angle: 35.,
            legend: false,
            date: String::new(),
        }
    }
}
impl ReadingMap {
    pub fn validate(&self) -> Result<(), String> {
        self.optical.validate()?;
        if !(2..=96).contains(&self.levels) {
            return Err("El número de isolíneas debe estar entre 2 y 96.".into());
        }
        if self.encoding == Encoding::Optical
            && (!self.optical.inks.is_empty() && self.optical.inks.len() != self.pens.len())
        {
            return Err("La mezcla óptica necesita una tinta física por familia.".into());
        }
        if (self.ramp == Ramp::Diverging && self.pens.len() < 2)
            || self.pens.is_empty()
            || self.pens.len() > 16
            || !(8..=128).contains(&self.samples)
            || !self.spacing_mm.is_finite()
            || !(0.1..=8.).contains(&self.spacing_mm)
            || !self.angle.is_finite()
            || self.weights.iter().any(|w| !w.is_finite() || w.abs() > 10.)
            || self.date.len() > 128
            || self
                .range
                .is_some_and(|[lo, hi]| !lo.is_finite() || !hi.is_finite() || hi <= lo)
            || self.threshold.is_some_and(|v| !v.is_finite())
        {
            return Err("Parámetros del mapa de lectura fuera de rango.".into());
        }
        if self.encoding == Encoding::Density
            && (self.pens.len() != 1 || self.ramp != Ramp::Sequential)
        {
            return Err("La densidad usa una tinta y una escala secuencial.".into());
        }
        if (self.measure == Measure::Objects || self.secondary == Some(Measure::Objects))
            && (self.ramp != Ramp::Categorical
                || self.secondary.is_some()
                || self.encoding != Encoding::Bands
                || self.equalize
                || self.invert
                || self.threshold.is_some())
        {
            return Err(
                "Los objetos requieren una rampa categórica, sin mezclar medidas ni normalizar."
                    .into(),
            );
        }
        if self.ramp == Ramp::Categorical && self.measure != Measure::Objects {
            return Err("La rampa categórica se reserva a objetos.".into());
        }
        Ok(())
    }
    pub fn target_color(&self, t: f64) -> [u8; 3] {
        let colors = self.optical.colors;
        if self.ramp != Ramp::Diverging {
            return colors.color(t);
        }
        let (end, amount) = if t < 0.5 {
            (colors.color(0.), 1. - t.clamp(0., 0.5) * 2.)
        } else {
            (
                if colors == ColorMap::Gray {
                    colors.color(0.)
                } else {
                    colors.color(1.)
                },
                (t.clamp(0.5, 1.) - 0.5) * 2.,
            )
        };
        std::array::from_fn(|i| (255. * (1. - amount) + end[i] as f64 * amount).round() as u8)
    }
    pub fn bind_palette(&mut self, palette: &[crate::Pen]) {
        if self.encoding == Encoding::Optical {
            self.optical.inks = self
                .pens
                .iter()
                .filter_map(|i| palette.get(*i))
                .map(|p| OpticalInk {
                    color: p.color,
                    width_mm: p.width_mm,
                })
                .collect();
        }
    }
    pub fn ordered_pens(&self, palette: &[crate::Pen]) -> Vec<usize> {
        let mut pens = self.pens.clone();
        if self.ramp == Ramp::Sequential && self.encoding != Encoding::Optical {
            pens.sort_by(|a, b| {
                let l = |i: &usize| palette.get(*i).map_or(0., |p| lightness(p.color));
                l(a).total_cmp(&l(b)).then(a.cmp(b))
            });
        }
        pens
    }
}
/// Luminosidad L de OKLab a partir de sRGB, sin confundirla con luminancia RGB.
pub fn lightness(rgb: [u8; 3]) -> f64 {
    let linear = |v: u8| {
        let v = v as f64 / 255.;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let [r, g, b] = rgb.map(linear);
    0.2104542553 * (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt()
        + 0.793617785 * (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt()
        - 0.0040720468 * (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt()
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ReadingEffect {
    Voxels { divisions: usize, hatch: bool, spacing_mm: f64 },
    Surface,
    Relief {
        length_mm: f64,
    },
    Needles {
        size_mm: f64,
    },
    Horizons {
        height_mm: f64,
        rows: usize,
    },
    Ridges {
        threshold: f64,
    },
    Suggestive {
        threshold: f64,
        min_angle_deg: f64,
        smoothing: usize,
    },
    ApparentRidges {
        threshold: f64,
        smoothing: usize,
    },
    PrincipalFlow {
        minimum: bool,
        spacing_mm: f64,
        length_mm: f64,
        smoothing: usize,
    },
    Anaglyph {
        angle_deg: f64,
        side_by_side: bool,
    },
    Unfold {
        tabs_mm: f64,
    },
    Spin {
        angle_deg: f64,
        copies: usize,
    },
    Section {
        direction: [f64; 3],
        offset: f64,
        #[serde(default = "show_dimensions_default")]
        show_dimensions: bool,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReadingParams {
    pub facets: usize,
    pub effect: ReadingEffect,
}
impl Default for ReadingParams {
    fn default() -> Self {
        Self {
            facets: 8000,
            effect: ReadingEffect::Surface,
        }
    }
}
fn show_dimensions_default() -> bool {
    true
}
impl ReadingParams {
    pub fn validate(&self) -> Result<(), String> {
        let valid = match self.effect {
            ReadingEffect::Voxels { divisions, spacing_mm, .. } => (4..=36).contains(&divisions) && (0.15..=3.).contains(&spacing_mm),
            ReadingEffect::Surface => true,
            ReadingEffect::Relief { length_mm } | ReadingEffect::Needles { size_mm: length_mm } => {
                length_mm.is_finite() && (0.2..=12.).contains(&length_mm)
            }
            ReadingEffect::Horizons { height_mm, rows } => {
                height_mm.is_finite()
                    && (-80.0..=80.).contains(&height_mm)
                    && (4..=240).contains(&rows)
            }
            ReadingEffect::Ridges { threshold } => {
                threshold.is_finite() && (0.0..=10.).contains(&threshold)
            }
            ReadingEffect::Suggestive {
                threshold,
                min_angle_deg,
                smoothing,
            } => {
                threshold.is_finite()
                    && (0.0..=20.).contains(&threshold)
                    && min_angle_deg.is_finite()
                    && (0.0..=80.).contains(&min_angle_deg)
                    && smoothing <= 5
            }
            ReadingEffect::ApparentRidges {
                threshold,
                smoothing,
            } => threshold.is_finite() && (0.0..=100.).contains(&threshold) && smoothing <= 5,
            ReadingEffect::Anaglyph { angle_deg, .. } => {
                angle_deg.is_finite() && (0.5..=8.).contains(&angle_deg)
            }
            ReadingEffect::PrincipalFlow {
                spacing_mm,
                length_mm,
                smoothing,
                ..
            } => {
                spacing_mm.is_finite()
                    && (0.6..=8.).contains(&spacing_mm)
                    && length_mm.is_finite()
                    && (2.0..=80.).contains(&length_mm)
                    && smoothing <= 5
            }
            ReadingEffect::Unfold { tabs_mm } => {
                tabs_mm.is_finite() && (0.5..=8.).contains(&tabs_mm) && self.facets <= 512
            }
            ReadingEffect::Spin { angle_deg, copies } => {
                angle_deg.is_finite() && angle_deg.abs() <= 180. && (2..=12).contains(&copies)
            }
            ReadingEffect::Section {
                direction, offset, ..
            } => {
                direction.iter().all(|v| v.is_finite() && v.abs() <= 1.)
                    && v3(direction).length() > 1e-9
                    && offset.is_finite()
                    && (-0.95..=0.95).contains(&offset)
            }
        };
        if valid && (64..=8000).contains(&self.facets) {
            Ok(())
        } else {
            Err("Parámetros de la pasada de lectura fuera de rango.".into())
        }
    }
}
#[derive(Default)]
pub(super) struct Cache {
    fields: fields::Cache,
    features: features::Cache,
    results: Vec<ResultEntry>,
    surfaces: Vec<(
        Arc<Mesh>,
        Camera,
        f64,
        Option<(f64, f64)>,
        Arc<surface::Surface>,
    )>,
    light: super::shadow::Cache,
}
impl Cache {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    fn surface(
        &mut self,
        d: &Drawing,
        s: &Settings,
        with_light: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Arc<surface::Surface>, String> {
        let key = with_light.then_some((s.shadow.azimuth, s.shadow.elevation));
        if let Some((_, _, _, _, surface)) = self.surfaces.iter().find(|(m, c, r, k, _)| {
            Arc::ptr_eq(m, &d.mesh) && *c == d.camera && *r == s.paper_rotation && *k == key
        }) {
            return Ok(surface.clone());
        }
        let light = if with_light {
            Some(self.light.field(
                d.mesh.clone(),
                &ShadowSettings {
                    threshold: 0.,
                    middle_tone: false,
                    ..s.shadow.clone()
                },
                cancel,
            )?)
        } else {
            None
        };
        self.light.retain_programs(&[]);
        let surface = Arc::new(surface::Surface::new(
            d,
            s,
            light.as_deref().map(|v| v.as_slice()),
            cancel,
        )?);
        if self.surfaces.len() >= 4 {
            self.surfaces.remove(0);
        }
        self.surfaces.push((
            d.mesh.clone(),
            d.camera,
            s.paper_rotation,
            key,
            surface.clone(),
        ));
        Ok(surface)
    }
}

struct ResultEntry {
    mesh: Arc<Mesh>,
    camera: Camera,
    scene: Settings,
    pass: Pass,
    paper: crate::Paper,
    full_paper: crate::Paper,
    row: usize,
    lines: Arc<Vec<Vec<Polyline>>>,
}
impl Cache {
    pub fn process(
        &mut self,
        d: &Drawing,
        input: &[Polyline],
        pass: &Pass,
        s: &Settings,
        paper: &crate::Paper,
        full_paper: &crate::Paper,
        row: usize,
        tones: &mut super::shadow::tone::ToneCache,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Vec<Vec<Polyline>>, String> {
        if d.mesh.triangles.len() > 100_000 || d.mesh.vertices.len() > 200_000 {
            return Err("El mapa de lectura admite hasta 100.000 caras de trabajo; reduce las facetas o la calidad.".into());
        }
        let map = pass
            .reading
            .as_ref()
            .ok_or("Falta la medida de la pasada.")?;
        let mut key = pass.clone();
        key.name.clear();
        key.pen = 0;
        key.enabled = true;
        key.finish = FinishSettings {
            halo_mm: key.finish.halo_mm,
            ..Default::default()
        };
        key.reading.as_mut().unwrap().pens = (0..map.pens.len()).collect();
        if let Some(result) = self.results.iter().find(|e| {
            Arc::ptr_eq(&e.mesh, &d.mesh)
                && e.camera == d.camera
                && e.scene == *s
                && e.pass == key
                && e.paper == *paper
                && e.full_paper == *full_paper
                && e.row == row
        }) {
            return Ok(result.lines.as_ref().clone());
        }
        let values = fields::Values::new(&mut self.fields, d, s, map, cancel)?;
        let geometry = self.fields.geometry(d.mesh.clone(), cancel)?;
        let needs_surface = !matches!(
            &pass.kind,
            PassKind::Reading(ReadingParams {
                effect: ReadingEffect::Voxels { .. }
                    | ReadingEffect::Ridges { .. }
                    | ReadingEffect::Suggestive { .. }
                    | ReadingEffect::ApparentRidges { .. }
                    | ReadingEffect::PrincipalFlow { .. }
                    | ReadingEffect::Anaglyph { .. }
                    | ReadingEffect::Spin { .. }
                    | ReadingEffect::Unfold { .. }
                    | ReadingEffect::Section { .. },
                ..
            })
        );
        let surface = if needs_surface {
            Some(self.surface(d, s, values.light_weight != 0., cancel)?)
        } else {
            None
        };
        let mut direct = None;
        let mut legend_scale = d.camera.scale;
        let lines = if let PassKind::Reading(params) = &pass.kind {
            match &params.effect {
                ReadingEffect::Voxels { divisions, hatch, spacing_mm } => {
                    direct = Some(voxels::draw(d, s, &geometry, *divisions, *hatch, *spacing_mm, paper, cancel)?);
                    vec![]
                }
                ReadingEffect::Surface
                    if matches!(map.encoding, Encoding::Optical | Encoding::Isolines) =>
                {
                    vec![]
                }
                ReadingEffect::Surface => {
                    effects::grid(effects::area(paper), map.spacing_mm, map.angle, cancel)?
                }
                ReadingEffect::Relief { .. } | ReadingEffect::Needles { .. } => effects::marks(
                    d,
                    s,
                    map,
                    &params.effect,
                    &values,
                    &geometry,
                    surface.as_ref().unwrap(),
                    paper,
                    cancel,
                )?,
                ReadingEffect::Horizons { height_mm, rows } => {
                    direct = Some(effects::horizons(
                        d,
                        map,
                        *height_mm,
                        *rows,
                        &values,
                        &geometry,
                        surface.as_ref().unwrap(),
                        paper,
                        cancel,
                    )?);
                    vec![]
                }
                ReadingEffect::Ridges { threshold } => {
                    let lines = self
                        .features
                        .ridges(d, &geometry.normals, *threshold, cancel)?;
                    direct = Some(effects::ridges(d, s, map, lines, paper, cancel)?);
                    vec![]
                }
                ReadingEffect::Suggestive { .. }
                | ReadingEffect::ApparentRidges { .. }
                | ReadingEffect::PrincipalFlow { .. } => {
                    let lines = self.features.draw(
                        d,
                        s,
                        &geometry.normals,
                        &params.effect,
                        paper,
                        cancel,
                    )?;
                    let mut groups = vec![vec![]; map.pens.len()];
                    groups[0] = lines;
                    direct = Some(groups);
                    vec![]
                }
                ReadingEffect::Anaglyph { .. } | ReadingEffect::Spin { .. } => {
                    if matches!(
                        params.effect,
                        ReadingEffect::Anaglyph {
                            side_by_side: true,
                            ..
                        }
                    ) {
                        legend_scale *= 0.48;
                    }
                    direct = Some(effects::views(d, s, map, &params.effect, paper, cancel)?);
                    vec![]
                }
                ReadingEffect::Unfold { tabs_mm } => {
                    let (groups, scale) = unfold::draw(d, map, *tabs_mm, paper, cancel)?;
                    legend_scale = scale;
                    direct = Some(groups);
                    vec![]
                }
                ReadingEffect::Section {
                    direction,
                    offset,
                    show_dimensions,
                } => {
                    direct = Some(section::draw(
                        d,
                        s,
                        map,
                        *direction,
                        *offset,
                        *show_dimensions,
                        paper,
                        cancel,
                    )?);
                    vec![]
                }
            }
        } else {
            input.to_vec()
        };
        let mut out = if map.encoding == Encoding::Optical {
            let mut groups = optical::draw(
                surface.as_ref().unwrap(),
                d,
                &values,
                &geometry,
                paper,
                cancel,
            )?;
            for group in &mut groups {
                *group = tones.apply(d, group, pass.mask, s, cancel)?;
            }
            groups
        } else if map.encoding == Encoding::Isolines {
            let mut groups =
                scalar::draw(surface.as_ref().unwrap(), d, &values, &geometry, s, cancel)?;
            for group in &mut groups {
                *group = tones.apply(d, group, pass.mask, s, cancel)?;
            }
            groups
        } else if let Some(mut groups) = direct {
            if pass.mask != TonalMask::All
                && !matches!(
                    pass.kind,
                    PassKind::Reading(ReadingParams {
                        effect: ReadingEffect::Unfold { .. }
                            | ReadingEffect::Section { .. }
                            | ReadingEffect::Anaglyph { .. }
                            | ReadingEffect::Spin { .. }
                            | ReadingEffect::Horizons { .. },
                        ..
                    })
                )
            {
                for lines in &mut groups {
                    *lines = tones.apply(d, lines, pass.mask, s, cancel)?;
                }
            }
            groups
        } else {
            let lines = tones.apply(d, &lines, pass.mask, s, cancel)?;
            encode::lines(
                surface.as_ref().unwrap(),
                d,
                &lines,
                &values,
                &geometry,
                cancel,
            )?
        };
        for lines in &mut out {
            *lines = encode::clip(lines, effects::area(paper));
        }
        if map.legend {
            legend::add(
                &mut out,
                map,
                &values,
                d,
                s,
                full_paper,
                row,
                &pass.kind,
                legend_scale,
            );
        }
        if out.iter().map(Vec::len).sum::<usize>() > 250_000 {
            return Err("El mapa supera 250.000 trazos; reduce la densidad.".into());
        }
        if self.results.len() >= 32 {
            self.results.remove(0);
        }
        self.results.push(ResultEntry {
            mesh: d.mesh.clone(),
            camera: d.camera,
            scene: s.clone(),
            pass: key,
            paper: paper.clone(),
            full_paper: full_paper.clone(),
            row,
            lines: Arc::new(out.clone()),
        });
        Ok(out)
    }
}

/// Papel reservado al modelo; las cartelas quedan fuera de la composición manipulable.
pub fn reading_content_paper<'a>(
    paper: &crate::Paper,
    passes: impl IntoIterator<Item = &'a Pass>,
) -> crate::Paper {
    let mut content = paper.clone();
    content.height_mm -= passes
        .into_iter()
        .filter(|p| p.reading.as_ref().is_some_and(|m| m.legend))
        .count() as f64
        * 26.;
    content
}

pub(super) fn clip_lines(lines: &[crate::Polyline], area: crate::BBox) -> Vec<crate::Polyline> {
    encode::clip(lines, area)
}
