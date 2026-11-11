//! Documento de Vectorio: papel, plumillas (rotuladores) y objetos editables.

use crate::fill::{self, Fill};
use crate::font::{self, TextSpec};
use crate::geom::{dist_to_lines, inside_rings, BBox, Placement, Polyline, P};
use crate::optimize;
use crate::shapes::Shape;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pen {
    pub name: String,
    pub color: [u8; 3],
    /// Ancho del trazo físico en mm.
    pub width_mm: f64,
}

impl Pen {
    pub fn new(name: &str, color: [u8; 3], width_mm: f64) -> Self {
        Pen { name: name.into(), color, width_mm }
    }
}

pub fn default_pens() -> Vec<Pen> {
    vec![
        Pen::new("Negro fino", [28, 28, 32], 0.4),
        Pen::new("Rojo", [214, 48, 49], 0.8),
        Pen::new("Azul", [9, 92, 196], 0.8),
        Pen::new("Verde", [0, 148, 96], 0.8),
        Pen::new("Amarillo", [247, 200, 30], 1.0),
        Pen::new("Naranja", [245, 124, 32], 1.0),
        Pen::new("Rosa", [232, 88, 160], 1.0),
        Pen::new("Violeta", [118, 68, 186], 0.8),
    ]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Paper {
    pub name: String,
    pub width_mm: f64,
    pub height_mm: f64,
    pub margin_mm: f64,
}

impl Paper {
    pub fn presets() -> Vec<Paper> {
        let mk = |n: &str, w, h| Paper { name: n.into(), width_mm: w, height_mm: h, margin_mm: 15.0 };
        vec![
            mk("A3 apaisado", 420.0, 297.0),
            mk("A3 vertical", 297.0, 420.0),
            mk("A4 apaisado", 297.0, 210.0),
            mk("A4 vertical", 210.0, 297.0),
            mk("A5 apaisado", 210.0, 148.0),
            mk("Postal", 148.0, 105.0),
        ]
    }
}

impl Default for Paper {
    fn default() -> Self {
        Paper::presets().remove(0)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlotSettings {
    /// Velocidad con la pluma bajada, mm/s.
    pub draw_speed: f64,
    /// Velocidad de desplazamiento con la pluma subida, mm/s.
    pub travel_speed: f64,
    /// Tiempo de subir y bajar la pluma, s.
    pub lift_time: f64,
    /// Ordena los trazos de cada plumilla para reducir desplazamientos.
    pub optimize: bool,
    /// Une trazos cuyos extremos coinciden (tolerancia en mm; 0 = no unir).
    pub join_tolerance: f64,
}

impl Default for PlotSettings {
    fn default() -> Self {
        PlotSettings { draw_speed: 40.0, travel_speed: 120.0, lift_time: 0.3, optimize: true, join_tolerance: 0.05 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Geometry {
    /// Trazados libres (mano alzada, vectorizado o SVG importado).
    Path { lines: Vec<Polyline>, closed: bool },
    Text(TextSpec),
    Shape(Shape),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: u64,
    pub name: String,
    pub geometry: Geometry,
    pub place: Placement,
    /// Plumilla del contorno; `None` = sin contorno.
    pub stroke: Option<usize>,
    pub fill: Fill,
    pub visible: bool,
    pub locked: bool,
}

impl Item {
    pub fn new(name: impl Into<String>, geometry: Geometry, pos: P) -> Self {
        Item {
            id: 0,
            name: name.into(),
            geometry,
            place: Placement { pos, ..Default::default() },
            stroke: Some(0),
            fill: Fill::default(),
            visible: true,
            locked: false,
        }
    }

    /// Trazos locales y si forman figuras cerradas (rellenables).
    pub fn local_lines(&self) -> (Vec<Polyline>, bool) {
        match &self.geometry {
            Geometry::Path { lines, closed } => (lines.clone(), *closed),
            Geometry::Text(spec) => (font::layout(spec), false),
            Geometry::Shape(s) => (vec![s.outline()], s.is_closed()),
        }
    }

    pub fn world_lines(&self) -> Vec<Polyline> {
        self.place.apply_all(&self.local_lines().0)
    }

    pub fn is_fillable(&self) -> bool {
        self.local_lines().1
    }

    pub fn bbox(&self) -> BBox {
        BBox::of(self.world_lines().iter())
    }

    /// ¿Toca el punto (mundo) el objeto? Dentro de figuras cerradas o cerca de un trazo.
    pub fn hit(&self, q: P, tol: f64) -> bool {
        let lines = self.world_lines();
        if !BBox::of(lines.iter()).contains(q, tol) {
            return false;
        }
        (self.is_fillable() && inside_rings(q, &lines)) || dist_to_lines(q, &lines) <= tol
    }

    /// Trazos de plotter por plumilla: primero el relleno y después el contorno.
    pub fn plot(&self, pens: &[Pen]) -> Vec<(usize, Vec<Polyline>)> {
        if !self.visible {
            return vec![];
        }
        let (local, closed) = self.local_lines();
        let world = self.place.apply_all(&local);
        let mut out = Vec::new();
        if closed && self.fill.style != fill::FillStyle::None && self.fill.pen < pens.len() {
            let w = pens[self.fill.pen].width_mm;
            let lines = fill::generate(&world, &self.fill, w, self.id);
            if !lines.is_empty() {
                out.push((self.fill.pen, lines));
            }
        }
        if let Some(pen) = self.stroke.filter(|p| *p < pens.len()) {
            out.push((pen, world));
        }
        out
    }
}

/// Trazos de una plumilla en orden de dibujo.
#[derive(Clone, Debug, Default)]
pub struct PlotLayer {
    pub pen: usize,
    pub paths: Vec<Polyline>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub paper: Paper,
    pub pens: Vec<Pen>,
    pub items: Vec<Item>,
    pub plot: PlotSettings,
    pub next_id: u64,
}

impl Default for Document {
    fn default() -> Self {
        Document { paper: Paper::default(), pens: default_pens(), items: vec![], plot: PlotSettings::default(), next_id: 1 }
    }
}

impl Document {
    pub fn add(&mut self, mut item: Item) -> u64 {
        item.id = self.next_id;
        self.next_id += 1;
        let id = item.id;
        self.items.push(item);
        id
    }

    pub fn index_of(&self, id: u64) -> Option<usize> {
        self.items.iter().position(|i| i.id == id)
    }

    pub fn get(&self, id: u64) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Item> {
        self.items.iter_mut().find(|i| i.id == id)
    }

    /// Al borrar una plumilla, los objetos que la usaban pasan a la primera.
    pub fn remove_pen(&mut self, idx: usize) {
        if self.pens.len() <= 1 || idx >= self.pens.len() {
            return;
        }
        self.pens.remove(idx);
        let fix = |p: usize| if p == idx { 0 } else if p > idx { p - 1 } else { p };
        for it in &mut self.items {
            it.stroke = it.stroke.map(fix);
            it.fill.pen = fix(it.fill.pen);
        }
    }

    /// Intercambia dos plumillas manteniendo los colores de cada objeto.
    pub fn swap_pens(&mut self, a: usize, b: usize) {
        if a >= self.pens.len() || b >= self.pens.len() {
            return;
        }
        self.pens.swap(a, b);
        let fix = |p: usize| if p == a { b } else if p == b { a } else { p };
        for it in &mut self.items {
            it.stroke = it.stroke.map(fix);
            it.fill.pen = fix(it.fill.pen);
        }
    }

    /// Mueve una plumilla a otra posición de la lista (orden de dibujo) y reordena los
    /// objetos para que el lienzo coincida: lo que va con una plumilla posterior queda
    /// encima. Devuelve el nuevo índice de cada plumilla antigua.
    pub fn move_pen(&mut self, from: usize, to: usize) -> Vec<usize> {
        let n = self.pens.len();
        let mut order: Vec<usize> = (0..n).collect();
        if from < n && to < n && from != to {
            let p = order.remove(from);
            order.insert(to, p);
        }
        let mut new_of = vec![0; n];
        for (new, old) in order.iter().enumerate() {
            new_of[*old] = new;
        }
        let old = std::mem::take(&mut self.pens);
        self.pens = order.iter().map(|o| old[*o].clone()).collect();
        let fix = |p: usize| new_of.get(p).copied().unwrap_or(p);
        for it in &mut self.items {
            it.stroke = it.stroke.map(fix);
            it.fill.pen = fix(it.fill.pen);
        }
        self.sort_items_by_pen();
        new_of
    }

    /// Plumilla principal de un objeto: la del relleno si está relleno, si no la del
    /// contorno.
    pub fn item_pen(it: &Item) -> usize {
        if it.is_fillable() && it.fill.style != fill::FillStyle::None {
            it.fill.pen
        } else {
            it.stroke.unwrap_or(it.fill.pen)
        }
    }

    /// Ordena los objetos por plumilla (orden estable dentro de cada una).
    pub fn sort_items_by_pen(&mut self) {
        self.items.sort_by_cached_key(Document::item_pen);
    }

    /// Plumilla más parecida a un color (distancia perceptual sencilla).
    pub fn nearest_pen(&self, c: [u8; 3]) -> usize {
        let d = |a: [u8; 3]| {
            let (dr, dg, db) = (a[0] as f64 - c[0] as f64, a[1] as f64 - c[1] as f64, a[2] as f64 - c[2] as f64);
            2.0 * dr * dr + 4.0 * dg * dg + 3.0 * db * db
        };
        (0..self.pens.len()).min_by(|&i, &j| d(self.pens[i].color).total_cmp(&d(self.pens[j].color))).unwrap_or(0)
    }

    /// Trazos de plotter de cada objeto (relleno y contorno), en paralelo: un objeto
    /// por núcleo.
    pub fn plot_items(&self) -> Vec<Vec<(usize, Vec<Polyline>)>> {
        self.items.par_iter().map(|it| it.plot(&self.pens)).collect()
    }

    /// Capas de plotter, una por plumilla en el orden de la lista de plumillas.
    pub fn layers(&self) -> Vec<PlotLayer> {
        self.layers_from(self.plot_items())
    }

    /// Agrupa por plumilla los trazos calculados con [`Document::plot_items`] y los
    /// ordena para reducir desplazamientos (cada capa en un núcleo).
    pub fn layers_from(&self, per_item: Vec<Vec<(usize, Vec<Polyline>)>>) -> Vec<PlotLayer> {
        let mut layers: Vec<PlotLayer> = (0..self.pens.len()).map(|pen| PlotLayer { pen, paths: vec![] }).collect();
        for parts in per_item {
            for (pen, lines) in parts {
                if let Some(layer) = layers.get_mut(pen) {
                    layer.paths.extend(lines.into_iter().filter(|l| l.len() >= 2));
                }
            }
        }
        if self.plot.optimize {
            let tol = self.plot.join_tolerance;
            layers.par_iter_mut().for_each(|l| {
                let ordered = optimize::order(std::mem::take(&mut l.paths), P::default());
                l.paths = if tol > 0.0 { optimize::join(ordered, tol) } else { ordered };
            });
        }
        layers.retain(|l| !l.paths.is_empty());
        layers
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn from_json(s: &str) -> Result<Document, String> {
        serde_json::from_str(s).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(pen: usize) -> Item {
        let mut it = Item::new(format!("p{pen}"), Geometry::Path { lines: vec![vec![crate::geom::p(0.0, 0.0), crate::geom::p(1.0, 1.0)]], closed: false }, P::default());
        it.stroke = Some(pen);
        it
    }

    #[test]
    fn move_pen_keeps_colors_and_restacks_items() {
        let mut d = Document::default();
        d.pens.truncate(3);
        let colors: Vec<_> = d.pens.iter().map(|p| p.color).collect();
        for pen in [2, 0, 1] {
            d.add(line(pen));
        }
        // La primera plumilla pasa al final: se dibuja la última y queda encima.
        let new_of = d.move_pen(0, 2);
        assert_eq!(new_of, vec![2, 0, 1]);
        assert_eq!(d.pens[2].color, colors[0]);
        // Cada objeto sigue con el mismo color y están ordenados por plumilla.
        let got: Vec<_> = d.items.iter().map(|i| (i.name.clone(), d.pens[i.stroke.unwrap()].color)).collect();
        assert_eq!(got, vec![("p1".into(), colors[1]), ("p2".into(), colors[2]), ("p0".into(), colors[0])]);
    }
}
