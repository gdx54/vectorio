//! Recetas vivas, cámaras y composición de varias vistas, siempre en milímetros.
use super::*;
use crate::{p, Document, Placement};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectionSettings {
    pub perspective: bool,
    /// Distancia al centro en diámetros del modelo; focal equivalente para el encuadre.
    pub distance: f64,
    /// Plano cercano en radios del modelo.
    pub near: f64,
}
impl Default for ProjectionSettings {
    fn default() -> Self {
        Self {
            perspective: false,
            distance: 3.,
            near: 0.05,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SheetViews {
    #[default]
    Single,
    Orthographic,
    Turntable,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VolumeObject {
    pub id: u64,
    pub name: String,
    pub recipe: Recipe,
    #[serde(default)]
    pub bindings: Vec<VolumeBinding>,
    /// Transformación en Diseño, independiente del encuadre de la receta.
    #[serde(default)]
    pub placement: Placement,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VolumeBinding {
    pub pass: usize,
    pub slot: Option<usize>,
    pub item: u64,
}
pub(super) fn configure_camera(camera: &mut Camera, s: &Settings, radius: f64) {
    camera.perspective = s.projection.perspective.then_some([
        2. * radius.max(1e-9) * s.projection.distance,
        radius.max(1e-9) * s.projection.near,
    ]);
}
impl Engine {
    pub(super) fn draw_sheet(
        &mut self,
        source: Arc<Mesh>,
        scene: &Settings,
        passes: &[Pass],
        paper: &crate::Paper,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Drawing, String> {
        let views: Vec<(f64, f64)> = match scene.sheet {
            SheetViews::Single => unreachable!(),
            SheetViews::Orthographic => {
                vec![(0., 89.99), (-90., 0.01), (0., 0.01), (45., 35.26438968)]
            }
            SheetViews::Turntable => (0..scene.view_count)
                .map(|i| {
                    (
                        scene.yaw + i as f64 * 360. / scene.view_count as f64,
                        scene.elevation,
                    )
                })
                .collect(),
        };
        let cols = (views.len() as f64).sqrt().ceil() as usize;
        let rows = views.len().div_ceil(cols);
        let gap = scene.view_gap_mm;
        let width = (paper.width_mm - 2. * paper.margin_mm - gap * (cols - 1) as f64) / cols as f64;
        let height =
            (paper.height_mm - 2. * paper.margin_mm - gap * (rows - 1) as f64) / rows as f64;
        if width < 14. || height < 14. {
            return Err(
                "Las vistas no caben en el papel. Reduce el número o la separación.".into(),
            );
        }
        let cell = crate::Paper {
            name: paper.name.clone(),
            width_mm: width,
            height_mm: height,
            margin_mm: 2.,
        };
        self.views.resize_with(views.len(), Engine::default);
        self.views.truncate(views.len());
        let mut result: Option<Drawing> = None;
        for (i, (yaw, elev)) in views.into_iter().enumerate() {
            if cancel() {
                return Err("Cálculo cancelado.".into());
            }
            let mut s = scene.clone();
            s.sheet = SheetViews::Single;
            s.yaw = yaw;
            s.elevation = elev;
            if scene.sheet == SheetViews::Orthographic {
                s.projection.perspective = false;
            }
            let mut d = self.views[i].draw_passes(source.clone(), &s, passes, &cell, cancel)?;
            let shift = p(
                paper.margin_mm + (i % cols) as f64 * (width + gap),
                paper.margin_mm + (i / cols) as f64 * (height + gap),
            );
            for part in &mut d.parts {
                for line in &mut part.lines {
                    for q in line {
                        *q = *q + shift;
                    }
                }
                for family in &mut part.families {
                    for line in &mut family.lines {
                        for q in line {
                            *q = *q + shift;
                        }
                    }
                }
            }
            if let Some(result) = &mut result {
                for (a, b) in result.parts.iter_mut().zip(d.parts) {
                    a.lines.extend(b.lines);
                    for family in b.families {
                        if let Some(f) = a.families.iter_mut().find(|f| f.slot == family.slot) {
                            f.lines.extend(family.lines);
                        } else {
                            a.families.push(family);
                        }
                    }
                }
            } else {
                d.camera.paper_center = d.camera.paper_center + shift;
                result = Some(d);
            }
        }
        Ok(result.unwrap())
    }
}
impl Document {
    pub fn volume(&self, id: u64) -> Option<&VolumeObject> {
        self.volumes.iter().find(|v| v.id == id)
    }
    /// Sustitución atómica del grupo: conserva su posición en la pila y transformación.
    pub fn put_volume(
        &mut self,
        mut volume: VolumeObject,
        drawing: &Drawing,
        replacing: Option<u64>,
    ) -> Result<Vec<u64>, String> {
        let old = if let Some(id) = replacing {
            Some(
                self.volume(id)
                    .cloned()
                    .ok_or("El objeto 3D ya no está en el documento.")?,
            )
        } else {
            None
        };
        let members = old
            .as_ref()
            .map_or_else(Vec::new, |v| self.group_items(v.id));
        if members
            .iter()
            .filter_map(|id| self.get(*id))
            .any(|it| it.locked)
        {
            return Err("Desbloquea el objeto 3D antes de actualizarlo.".into());
        }
        let first = members.first().and_then(|id| self.get(*id));
        let place = first.map_or(volume.placement, |it| it.place);
        let visible = first.is_none_or(|it| it.visible);
        let index = members
            .first()
            .and_then(|id| self.index_of(*id))
            .unwrap_or(self.items.len());
        let mut items = drawing.items([]);
        if items.is_empty() {
            return Err(
                "El objeto 3D no contiene trazos. Activa una pasada antes de colocarlo.".into(),
            );
        }
        volume.id = old.as_ref().map_or(self.next_id, |v| v.id);
        if old.is_none() {
            self.next_id += 1;
        }
        volume.placement = place;
        let ids = items
            .iter_mut()
            .map(|it| {
                it.group = Some(volume.id);
                it.place = place;
                it.visible = visible;
                it.name = it.name.replace("VOLUMETRIC", "Volumen");
                it.id = self.next_id;
                self.next_id += 1;
                it.id
            })
            .collect::<Vec<_>>();
        volume.bindings.clear();
        let mut ids_iter = ids.iter();
        for ((pass_index, _), part) in volume
            .recipe
            .passes
            .iter()
            .enumerate()
            .filter(|(_, p)| p.enabled)
            .zip(&drawing.parts)
        {
            for (slot, (_, lines)) in part.inks().enumerate() {
                if !lines.is_empty() {
                    if let Some(id) = ids_iter.next() {
                        volume.bindings.push(VolumeBinding {
                            pass: pass_index,
                            slot: (!part.families.is_empty()).then(|| part.families[slot].slot),
                            item: *id,
                        });
                    }
                }
            }
        }
        self.items.retain(|it| !members.contains(&it.id));
        self.items.splice(index..index, items);
        if let Some(v) = self.volumes.iter_mut().find(|v| v.id == volume.id) {
            *v = volume;
        } else {
            self.volumes.push(volume);
        }
        Ok(ids)
    }
    /// Las tintas que se retocaron en Diseño se reflejan al reabrir el objeto.
    pub fn volume_recipe(&self, id: u64) -> Option<Recipe> {
        let volume = self.volume(id)?;
        let mut recipe = volume.recipe.clone();
        recipe.inks = self.pens.clone();
        for pass in &mut recipe.passes {
            if let Some(map) = &mut pass.reading {
                map.pens = map.ordered_pens(&self.pens);
            }
        }
        for b in &volume.bindings {
            if let (Some(pass), Some(pen)) = (
                recipe.passes.get_mut(b.pass),
                self.get(b.item).and_then(|it| it.stroke),
            ) {
                if let Some(slot) = b.slot {
                    if let Some(map) = &mut pass.reading {
                        if let Some(p) = map.pens.get_mut(slot) {
                            *p = pen;
                        } else {
                            pass.pen = pen;
                        }
                    }
                } else {
                    pass.pen = pen;
                }
            }
        }
        Some(recipe)
    }
    pub(crate) fn remap_volume_pens(&mut self, fix: impl Fn(usize) -> usize) {
        let pens = self.pens.clone();
        for v in &mut self.volumes {
            for pass in &mut v.recipe.passes {
                pass.pen = fix(pass.pen);
                if let Some(map) = &mut pass.reading {
                    for pen in &mut map.pens {
                        *pen = fix(*pen);
                    }
                }
            }
            v.recipe.inks = pens.clone();
        }
    }
}
