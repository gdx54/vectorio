//! Generadores deterministas independientes del documento y la interfaz.
use std::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};
use crate::geom::{length, BBox, Polyline, Rng};
mod blocks;
mod spiro;
mod document;
#[cfg(test)] mod tests;
pub use document::GeneratedGroup;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamValue { Number(f64), Text(String), Bool(bool) }
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Params(pub BTreeMap<String, ParamValue>);
impl Params {
    pub fn f64(&self, id: &str) -> f64 { match self.0.get(id) { Some(ParamValue::Number(n)) => *n, _ => 0.0 } }
    pub fn int(&self, id: &str) -> usize { self.f64(id).round().max(0.0) as usize }
    pub fn choice(&self, id: &str) -> &str { match self.0.get(id) { Some(ParamValue::Text(s)) => s, _ => "" } }
    pub fn flag(&self, id: &str) -> bool { matches!(self.0.get(id), Some(ParamValue::Bool(true))) }
    pub fn set(&mut self, id: &str, value: ParamValue) { self.0.insert(id.into(), value); }
    /// Migración por esquema: elimina desconocidos, repone ausentes y sanea tipos/rangos.
    pub fn normalized(&self, specs: &[ParamSpec]) -> Self {
        let mut out = Self::default();
        for spec in specs {
            let value = match spec {
                ParamSpec::Section { .. } => continue,
                ParamSpec::Range { id, min, max, default, .. } => {
                    let n = match self.0.get(*id) { Some(ParamValue::Number(n)) if n.is_finite() => *n, _ => *default };
                    ParamValue::Number(n.clamp(*min, *max))
                }
                ParamSpec::Choice { id, options, default, .. } => {
                    let s = self.choice(id);
                    ParamValue::Text(if options.iter().any(|(v, _)| *v == s) { s } else { default }.to_string())
                }
                ParamSpec::Toggle { id, default, .. } => self.0.get(*id).filter(|v| matches!(v, ParamValue::Bool(_)))
                    .cloned().unwrap_or(ParamValue::Bool(*default)),
            };
            out.set(spec.id().unwrap_or_default(), value);
        }
        out
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Show { pub id: &'static str, pub equals: &'static str }
#[derive(Clone, Debug)]
pub enum ParamSpec {
    Section { label: &'static str },
    Range { id: &'static str, label: &'static str, min: f64, max: f64, step: f64, default: f64,
        random: Option<(f64, f64)>, show: Option<Show>, hint: Option<&'static str> },
    Choice { id: &'static str, label: &'static str, options: &'static [(&'static str, &'static str)], default: &'static str,
        random: bool, show: Option<Show>, hint: Option<&'static str> },
    Toggle { id: &'static str, label: &'static str, default: bool, random: Option<f64>, show: Option<Show>, hint: Option<&'static str> },
}
impl ParamSpec {
    pub fn id(&self) -> Option<&'static str> { match self { Self::Section { .. } => None,
        Self::Range { id, .. } | Self::Choice { id, .. } | Self::Toggle { id, .. } => Some(id) } }
    pub fn label(&self) -> &'static str { match self { Self::Section { label } | Self::Range { label, .. } |
        Self::Choice { label, .. } | Self::Toggle { label, .. } => label } }
    pub fn hint(&self) -> Option<&'static str> { match self { Self::Section { .. } => None,
        Self::Range { hint, .. } | Self::Choice { hint, .. } | Self::Toggle { hint, .. } => *hint } }
    pub fn shown(&self, params: &Params) -> bool {
        let condition = match self { Self::Section { .. } => None, Self::Range { show, .. } |
            Self::Choice { show, .. } | Self::Toggle { show, .. } => *show };
        condition.is_none_or(|s| match params.0.get(s.id) {
            Some(ParamValue::Text(v)) => if let Some(value) = s.equals.strip_prefix('!') { v != value } else { v == s.equals }, Some(ParamValue::Bool(v)) => if let Some(value) = s.equals.strip_prefix('!') { v.to_string() != value } else { v.to_string() == s.equals },
            Some(ParamValue::Number(v)) => if let Some(value) = s.equals.strip_prefix('!') { v.to_string() != value } else { v.to_string() == s.equals }, None => false,
        })
    }
}
pub(super) fn range(id: &'static str, label: &'static str, min: f64, max: f64, step: f64, default: f64) -> ParamSpec {
    ParamSpec::Range { id, label, min, max, step, default, random: Some((min, max)), show: None, hint: None }
}
pub(super) fn choice(id: &'static str, label: &'static str, options: &'static [(&'static str, &'static str)], default: &'static str) -> ParamSpec {
    ParamSpec::Choice { id, label, options, default, random: true, show: None, hint: None }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Detail { Preview, Final }
#[derive(Clone, Debug, PartialEq)]
pub struct GenContext { pub area: BBox, pub seed: u64, pub pen_mm: f64, pub detail: Detail }
impl GenContext {
    pub fn valid(&self) -> bool { crate::scene::valid_area(self.area) && self.area.width() >= 1.0 && self.area.height() >= 1.0 &&
        self.pen_mm.is_finite() && self.pen_mm > 0.0 }
}
#[derive(Clone, Debug, PartialEq)]
pub struct GenPart { pub name: String, pub lines: Vec<Polyline>, pub pen_order: Option<usize> }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GenOutput { pub parts: Vec<GenPart>, pub warning: Option<String> }
impl GenOutput {
    pub fn lengths_m(&self) -> Vec<f64> { self.parts.iter().map(|part| part.lines.iter().map(|l| length(l)).sum::<f64>() / 1000.0).collect() }
    pub fn total_m(&self) -> f64 { self.lengths_m().iter().sum() }
}
impl From<crate::scene::SceneOutput> for GenOutput {
    fn from(out: crate::scene::SceneOutput) -> Self { Self { parts: out.parts.into_iter().map(|p| GenPart {
        name: p.name, lines: p.lines, pen_order: None }).collect(), warning: None } }
}
pub trait Generator: Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn category(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn params(&self) -> Vec<ParamSpec>;
    fn randomize(&self, rng: &mut Rng, params: &mut Params, locks: &BTreeSet<String>) {
        randomize_specs(&self.params(), rng, params, locks);
    }
    fn generate(&self, params: &Params, context: &GenContext) -> Result<GenOutput, String>;
}
static BLOCKS: blocks::Blocks = blocks::Blocks;
static SPIRO: spiro::Spiro = spiro::Spiro;
static ALL: [&'static dyn Generator; 2] = [&BLOCKS, &SPIRO];
pub fn all() -> &'static [&'static dyn Generator] { &ALL }
pub fn find(id: &str) -> Option<&'static dyn Generator> { ALL.iter().copied().find(|g| g.id() == id) }

/// Aleatoriedad declarativa reutilizable por generadores con restricciones adicionales.
pub fn randomize_specs(specs: &[ParamSpec], rng: &mut Rng, params: &mut Params, locks: &BTreeSet<String>) {
        *params = params.normalized(specs);
        for spec in specs {
            let Some(id) = spec.id() else { continue };
            if locks.contains(id) { continue; }
            let value = match spec {
                ParamSpec::Range { min, max, step, random: Some((lo, hi)), .. } => {
                    let n = lo + rng.f() * (hi - lo);
                    ParamValue::Number((min + ((n - min) / step).round() * step).clamp(*min, *max))
                }
                ParamSpec::Choice { options, random: true, .. } if !options.is_empty() =>
                    ParamValue::Text(options[(rng.f() * options.len() as f64) as usize].0.into()),
                ParamSpec::Toggle { random: Some(probability), .. } => ParamValue::Bool(rng.f() < *probability),
                _ => continue,
            };
            params.set(id, value);
        }
}
