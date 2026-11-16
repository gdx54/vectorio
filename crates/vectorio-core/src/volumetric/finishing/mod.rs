//! Acabado vectorial en mm. Se aplica después de máscara y reparto de tintas.
mod echo;
#[cfg(test)]
mod tests;
mod variation;
use super::*;
use crate::{p, BBox, Polyline};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FinishSettings {
    pub halo_mm: f64,
    pub echo_count: usize,
    pub echo_mm: f64,
    pub jitter_mm: f64,
    pub wavelength_mm: f64,
    pub irregularity: f64,
    pub gap_probability: f64,
    pub gap_mm: f64,
    pub seed: u64,
}
impl Default for FinishSettings {
    fn default() -> Self {
        Self {
            halo_mm: 0.,
            echo_count: 0,
            echo_mm: 0.3,
            jitter_mm: 0.,
            wavelength_mm: 3.,
            irregularity: 0.,
            gap_probability: 0.,
            gap_mm: 0.35,
            seed: 1,
        }
    }
}
impl FinishSettings {
    pub fn validate(&self) -> Result<(), String> {
        if [
            self.halo_mm,
            self.echo_mm,
            self.jitter_mm,
            self.wavelength_mm,
            self.irregularity,
            self.gap_probability,
            self.gap_mm,
        ]
        .iter()
        .any(|v| !v.is_finite())
            || !(0.0..=5.).contains(&self.halo_mm)
            || self.echo_count > 12
            || self.echo_mm.abs() > 4.
            || self.echo_count > 0 && self.echo_mm.abs() < 0.02
            || !(0.0..=1.).contains(&self.jitter_mm)
            || !(0.5..=20.).contains(&self.wavelength_mm)
            || !(0.0..=1.).contains(&self.irregularity)
            || !(0.0..=0.5).contains(&self.gap_probability)
            || !(0.1..=3.).contains(&self.gap_mm)
        {
            return Err("Parámetros de acabado artístico fuera de rango.".into());
        }
        Ok(())
    }
    pub fn variation(&self) -> bool {
        self.jitter_mm > 0. || self.irregularity > 0. || self.gap_probability > 0.
    }
    pub fn post(&self) -> bool {
        self.echo_count > 0 || self.variation()
    }
}
impl Pass {
    pub fn supports_halo(&self) -> bool {
        self.visibility == Visibility::Visible
            && matches!(
                self.kind,
                PassKind::Outline(_)
                    | PassKind::Edges { .. }
                    | PassKind::Field(_)
                    | PassKind::Waves(_)
                    | PassKind::Mesh(_)
            )
    }
    pub fn supports_echo(&self) -> bool {
        matches!(self.kind, PassKind::Outline(_))
    }
}
const ECHO_SLOT: usize = usize::MAX;
struct Entry {
    hash: u64,
    settings: FinishSettings,
    seed: u64,
    area: BBox,
    part: DrawingPart,
}
#[derive(Default)]
pub(super) struct Cache {
    entries: Vec<Entry>,
    #[cfg(test)]
    pub builds: usize,
}
fn hash(part: &DrawingPart) -> u64 {
    let mut h = variation::mix(part.families.len() as u64 ^ 0x859c2aa8cafe1234);
    let groups = if part.families.is_empty() {
        vec![(0, &part.lines)]
    } else {
        part.families.iter().map(|f| (f.slot, &f.lines)).collect()
    };
    for (slot, lines) in groups {
        h = variation::mix(h ^ slot as u64);
        for line in lines {
            h = variation::mix(h ^ line.len() as u64);
            for p in line {
                h = variation::mix(h ^ p.x.to_bits());
                h = variation::mix(h ^ p.y.to_bits());
            }
        }
    }
    h
}

impl Cache {
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    pub fn apply(
        &mut self,
        part: &mut DrawingPart,
        pass: &Pass,
        scene_seed: u64,
        paper: &crate::Paper,
        cancel: &dyn Fn() -> bool,
    ) -> Result<(), String> {
        let settings = &pass.finish;
        if !settings.post() {
            return Ok(());
        }
        let seed = variation::mix(scene_seed ^ settings.seed.rotate_left(23));
        let area = BBox {
            min: p(paper.margin_mm, paper.margin_mm),
            max: p(
                paper.width_mm - paper.margin_mm,
                paper.height_mm - paper.margin_mm,
            ),
        };
        let hash = hash(part);
        if let Some(entry) = self
            .entries
            .iter()
            .find(|e| e.hash == hash && e.settings == *settings && e.seed == seed && e.area == area)
        {
            part.lines = entry.part.lines.clone();
            part.families = entry.part.families.clone();
            return Ok(());
        }
        let preserve_legend = pass.reading.as_ref().is_some_and(|map| map.legend);
        if settings.echo_count > 0 {
            let model = part
                .lines
                .iter()
                .filter(|l| !preserve_legend || !legend(l, area))
                .cloned()
                .collect::<Vec<_>>();
            let copies = echo::copies(&model, settings.echo_count, settings.echo_mm, cancel)?;
            if part.families.is_empty() {
                part.lines.extend(copies);
            } else {
                part.families.push(InkFamily {
                    slot: ECHO_SLOT,
                    pen: pass.pen,
                    lines: copies,
                });
            }
        }
        if part.families.is_empty() {
            part.lines = decorate(&part.lines, settings, seed, area, preserve_legend, cancel)?;
        } else {
            for family in &mut part.families {
                family.lines = decorate(
                    &family.lines,
                    settings,
                    seed,
                    area,
                    preserve_legend && family.slot != ECHO_SLOT,
                    cancel,
                )?;
            }
            part.lines = part.families.iter().flat_map(|f| f.lines.clone()).collect();
        }
        if part.lines.len() > 250_000 || part.lines.iter().map(Vec::len).sum::<usize>() > 2_000_000
        {
            return Err(
                "El acabado supera el presupuesto de trazos; reduce ecos, temblor o densidad."
                    .into(),
            );
        }
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        if self.entries.len() >= 32 {
            self.entries.remove(0);
        }
        self.entries.push(Entry {
            hash,
            settings: settings.clone(),
            seed,
            area,
            part: part.clone(),
        });
        #[cfg(test)]
        {
            self.builds += 1;
        }
        Ok(())
    }
}
fn legend(line: &Polyline, area: BBox) -> bool {
    line.iter().all(|p| p.y > area.max.y + 0.5)
}
fn decorate(
    lines: &[Polyline],
    settings: &FinishSettings,
    seed: u64,
    area: BBox,
    preserve_legend: bool,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Polyline>, String> {
    let mut out = Vec::new();
    let mut points = 0;
    for line in lines {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        if preserve_legend && legend(line, area) {
            out.push(line.clone());
            continue;
        }
        let varied = variation::lines(line, settings, seed, cancel)?;
        let clipped = super::reading::clip_lines(&varied, area);
        points += clipped.iter().map(Vec::len).sum::<usize>();
        out.extend(clipped);
        if points > 2_000_000 || out.len() > 250_000 {
            return Err("Demasiados trazos de acabado; reduce la densidad.".into());
        }
    }
    Ok(out)
}
