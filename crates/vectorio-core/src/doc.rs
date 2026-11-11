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
    /// Un nombre elegido por el usuario no cambia al editar la tinta.
    #[serde(default)]
    pub custom_name: bool,
}

impl Pen {
    pub fn new(name: &str, color: [u8; 3], width_mm: f64) -> Self {
        Pen { name: name.into(), color, width_mm, custom_name: false }
    }

    /// Tinta con el nombre automático de su color («Rojo», «Marrón»…), que seguirá
    /// al color si luego se edita.
    pub fn automatic(color: [u8; 3], width_mm: f64) -> Self {
        Pen::new(color_name(color), color, width_mm)
    }

    pub fn set_color(&mut self, color: [u8; 3]) {
        if color == self.color { return; }
        let automatic = !self.custom_name && (matches!(self.name.as_str(),
            "Negro fino" | "Negro" | "Blanco" | "Gris" | "Rojo" | "Naranja" | "Amarillo" | "Verde" |
            "Turquesa" | "Cian" | "Azul" | "Índigo" | "Violeta" | "Morado" | "Magenta" | "Rosa" | "Marrón")
            || self.name.starts_with("Rotulador ") || self.name.starts_with("Tinta de "));
        self.color = color;
        if automatic { self.name = color_name(color).into(); }
    }
}

fn color_name([r, g, b]: [u8; 3]) -> &'static str {
    let (r, g, b) = (r as f32, g as f32, b as f32);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    if max < 48.0 { return "Negro"; }
    if max - min < 24.0 { return if min > 225.0 { "Blanco" } else { "Gris" }; }
    let d = max - min;
    let h = (if max == r { (g - b) / d } else if max == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 }) * 60.0;
    match h.rem_euclid(360.0) as u32 {
        0..=14 | 345..=359 => "Rojo",
        15..=44 => if max < 165.0 { "Marrón" } else { "Naranja" },
        45..=74 => "Amarillo", 75..=164 => "Verde", 165..=194 => "Turquesa",
        195..=254 => "Azul", 255..=294 => "Morado", _ => "Rosa",
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
    /// Trama de relleno ya vectorizada y fija (Mezcla de versiones anteriores, o tras
    /// «Convertir en trazos»). Son recorridos de tinta, no contornos.
    Screened { lines: Vec<Polyline>, screen: crate::mix::Screen },
    /// Tinta de una foto viva: mapa de cobertura y trama que se puede retocar.
    Traced(crate::trace::Traced),
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
    /// Foto viva a la que pertenece (sus tintas se agrupan en Capas).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<u64>,
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
            group: None,
        }
    }

    /// Trazos locales y si forman figuras cerradas (rellenables).
    pub fn local_lines(&self) -> (Vec<Polyline>, bool) {
        match &self.geometry {
            Geometry::Path { lines, closed } => (lines.clone(), *closed),
            Geometry::Screened { lines, .. } => (lines.clone(), false),
            Geometry::Traced(t) => (t.lines.to_vec(), false),
            Geometry::Text(spec) => (font::layout(spec), false),
            Geometry::Shape(s) => (vec![s.outline()], s.is_closed()),
        }
    }

    pub fn world_lines(&self) -> Vec<Polyline> {
        self.place.apply_all(&self.local_lines().0)
    }

    pub fn is_fillable(&self) -> bool {
        match &self.geometry {
            Geometry::Path { closed, .. } => *closed,
            Geometry::Shape(shape) => shape.is_closed(),
            Geometry::Text(_) | Geometry::Screened { .. } | Geometry::Traced(_) => false,
        }
    }

    /// ¿Es una trama de tinta (fija o viva)? Solo tiene rotulador de relleno.
    pub fn is_screened(&self) -> bool { matches!(self.geometry, Geometry::Screened { .. } | Geometry::Traced(_)) }

    pub fn traced(&self) -> Option<&crate::trace::Traced> {
        if let Geometry::Traced(t) = &self.geometry { Some(t) } else { None }
    }

    pub fn traced_mut(&mut self) -> Option<&mut crate::trace::Traced> {
        if let Geometry::Traced(t) = &mut self.geometry { Some(t) } else { None }
    }

    pub fn has_fill(&self) -> bool {
        self.is_screened() || (self.is_fillable() && self.fill.style != fill::FillStyle::None)
    }

    pub fn bbox(&self) -> BBox {
        if let Geometry::Text(spec) = &self.geometry {
            if spec.is_frame() {
                return BBox::of([&vec![self.place.apply(P::default()), self.place.apply(crate::p(spec.wrap_mm, 0.0)),
                    self.place.apply(crate::p(spec.wrap_mm, spec.frame_height_mm)), self.place.apply(crate::p(0.0, spec.frame_height_mm))]]);
            }
        }
        BBox::of(self.world_lines().iter())
    }

    /// ¿Toca el punto (mundo) el objeto? Dentro de figuras cerradas o cerca de un trazo.
    pub fn hit(&self, q: P, tol: f64) -> bool {
        if let Geometry::Text(spec) = &self.geometry {
            if spec.is_frame() {
                let local = self.place.inverse(q);
                let tol = tol / self.place.scale.max(1e-9);
                return local.x >= -tol && local.y >= -tol && local.x <= spec.wrap_mm + tol && local.y <= spec.frame_height_mm + tol;
            }
        }
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
        if self.is_screened() {
            if self.fill.pen < pens.len() { out.push((self.fill.pen, world)); }
            return out;
        }
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
    /// Fotos vivas (imagen y ajustes de separación) de los grupos de tintas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub photos: Vec<crate::photo::Photo>,
}

impl Default for Document {
    fn default() -> Self {
        Document { paper: Paper::default(), pens: default_pens(), items: vec![], plot: PlotSettings::default(), next_id: 1, photos: vec![] }
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

    /// Prepara una tinta independiente para editar solo el contorno o relleno de un
    /// objeto. Reutiliza la tinta si ya es exclusiva; conserva el grosor físico.
    pub fn isolate_ink(&mut self, id: u64, fill: bool) -> Option<usize> {
        let it = self.get(id).filter(|it| !it.locked && (if fill { it.is_fillable() || it.is_screened() } else { !it.is_screened() }))?;
        let old = if fill { it.fill.pen } else { it.stroke.unwrap_or(it.fill.pen) };
        let name = format!("Tinta de {}", it.name);
        let shared = self.items.iter().any(|other| {
            ((other.id != id || fill) && other.stroke == Some(old))
                || ((other.id != id || !fill) && other.fill.pen == old && other.has_fill())
        });
        let index = if shared || old >= self.pens.len() {
            let mut pen = self.pens.get(old).cloned().unwrap_or_else(|| Pen::new("Tinta", [60, 60, 70], 0.8));
            pen.name = name;
            let index = self.pens.len();
            self.pens.push(pen);
            index
        } else { old };
        let it = self.get_mut(id)?;
        if fill {
            it.fill.pen = index;
            if !it.is_screened() && it.fill.style == fill::FillStyle::None { it.fill.style = fill::FillStyle::Hatch; }
        } else { it.stroke = Some(index); }
        Some(index)
    }

    /// Asigna una tinta compartida de la paleta al contorno o al relleno de un objeto.
    /// Las tramas solo tienen relleno; un relleno vacío pasa a rayado para verse.
    pub fn assign_pen(&mut self, id: u64, pen: usize, fill: bool) -> bool {
        if pen >= self.pens.len() { return false; }
        let Some(it) = self.get_mut(id).filter(|it| !it.locked) else { return false };
        let fill = fill || it.is_screened();
        if fill {
            if !it.is_fillable() && !it.is_screened() { return false; }
            it.fill.pen = pen;
            if !it.is_screened() && it.fill.style == fill::FillStyle::None { it.fill.style = fill::FillStyle::Hatch; }
        } else {
            it.stroke = Some(pen);
        }
        true
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

    /// Reordena las tintas para el plotter sin alterar el orden visual de los objetos.
    /// Devuelve el nuevo índice de cada plumilla antigua.
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
        new_of
    }

    /// Con qué ajustes debe estar calculada cada tinta viva. Las tintas de una foto con
    /// relevo y mapas del mismo tamaño comparten las líneas de la primera de ellas, con
    /// el grosor del rotulador más ancho.
    pub fn trace_keys(&self) -> std::collections::HashMap<u64, crate::trace::TraceKey> {
        let width = |it: &Item| self.pens.get(it.fill.pen).map_or(0.6, |p| p.width_mm);
        let mut keys = std::collections::HashMap::new();
        let mut groups: Vec<(u64, Vec<&Item>)> = Vec::new();
        for it in &self.items {
            let Some(t) = it.traced() else { continue };
            keys.insert(it.id, t.key(width(it)));
            if let Some(g) = it.group.filter(|_| t.trace.relay) {
                match groups.iter_mut().find(|(id, _)| *id == g) {
                    Some((_, members)) => members.push(it),
                    None => groups.push((g, vec![it])),
                }
            }
        }
        for (_, members) in groups {
            let lead = members[0].traced().unwrap();
            let fits = |it: &&Item| it.traced().is_some_and(|t| {
                (t.coverage.map.w, t.coverage.map.h, t.coverage.mm_per_px) == (lead.coverage.map.w, lead.coverage.map.h, lead.coverage.mm_per_px)
            });
            let members: Vec<&Item> = members.into_iter().filter(fits).collect();
            if members.len() < 2 {
                continue;
            }
            let inks: Vec<_> = members.iter().map(|it| it.traced().unwrap().coverage.clone()).collect();
            let common = members.iter().map(|it| width(it)).fold(0.0, f64::max);
            for (index, it) in members.iter().enumerate() {
                if let Some(key) = keys.get_mut(&it.id) {
                    key.relay = Some(crate::trace::Relay { inks: inks.clone(), index, look: lead.trace.clone(), pen_width_mm: common, seed: lead.seed });
                }
            }
        }
        keys
    }

    /// Estos objetos y sus compañeros de relevo: al retocar el trazo de una tinta en
    /// relevo, cambian todas para que sigan compartiendo las líneas.
    pub fn with_relay_partners(&self, ids: &[u64]) -> Vec<u64> {
        let groups: Vec<u64> = ids.iter()
            .filter_map(|id| self.get(*id))
            .filter(|it| it.traced().is_some_and(|t| t.trace.relay))
            .filter_map(|it| it.group)
            .collect();
        self.items.iter()
            .filter(|it| ids.contains(&it.id) || (it.group.is_some_and(|g| groups.contains(&g)) && it.traced().is_some_and(|t| t.trace.relay)))
            .map(|it| it.id)
            .collect()
    }

    /// Orden de pasadas: reordena las plumillas de estos objetos para que el plotter
    /// pase primero las claras (o las oscuras). Ocupan los mismos huecos de la lista de
    /// tintas, así que las demás no se mueven; los objetos se dibujan en el mismo orden.
    pub fn order_passes(&mut self, ids: &[u64], light_first: bool) {
        let mut slots: Vec<usize> = ids.iter().filter_map(|id| self.get(*id)).map(Document::item_pen).filter(|p| *p < self.pens.len()).collect();
        slots.sort_unstable();
        slots.dedup();
        let lum = |c: [u8; 3]| 0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64;
        let mut by_tone = slots.clone();
        by_tone.sort_by(|a, b| lum(self.pens[*b].color).total_cmp(&lum(self.pens[*a].color)));
        if !light_first {
            by_tone.reverse();
        }
        let old = self.pens.clone();
        let mut new_of: Vec<usize> = (0..old.len()).collect();
        for (slot, src) in slots.iter().zip(&by_tone) {
            self.pens[*slot] = old[*src].clone();
            new_of[*src] = *slot;
        }
        for it in &mut self.items {
            it.stroke = it.stroke.map(|p| new_of.get(p).copied().unwrap_or(p));
            it.fill.pen = new_of.get(it.fill.pen).copied().unwrap_or(it.fill.pen);
        }
        // Dentro de sus posiciones, los objetos quedan en el orden de las pasadas.
        let places: Vec<usize> = (0..self.items.len()).filter(|i| ids.contains(&self.items[*i].id)).collect();
        let mut moving: Vec<Item> = places.iter().map(|i| self.items[*i].clone()).collect();
        moving.sort_by_key(Document::item_pen);
        for (i, it) in places.into_iter().zip(moving) {
            self.items[i] = it;
        }
    }

    /// Plumilla principal de un objeto: la del relleno si está relleno, si no la del
    /// contorno.
    pub fn item_pen(it: &Item) -> usize {
        if it.has_fill() {
            it.fill.pen
        } else {
            it.stroke.unwrap_or(it.fill.pen)
        }
    }

    /// Ordena los objetos por plumilla (orden estable dentro de cada una).
    pub fn sort_items_by_pen(&mut self) {
        self.items.sort_by_cached_key(Document::item_pen);
    }

    /// Mueve una selección en el orden visual, conservando sus tintas y orden relativo.
    /// `above` significa delante del objeto ancla (después en el vector).
    pub fn move_items(&mut self, ids: &[u64], anchor: Option<(u64, bool)>) {
        if anchor.is_some_and(|(id, _)| ids.contains(&id)) { return; }
        let mut moving = Vec::new();
        let mut rest = Vec::new();
        for it in std::mem::take(&mut self.items) {
            if ids.contains(&it.id) && !it.locked { moving.push(it); }
            else { rest.push(it); }
        }
        let index = anchor.and_then(|(id, above)| rest.iter().position(|it| it.id == id).map(|i| i + usize::from(above)))
            .unwrap_or(rest.len());
        rest.splice(index..index, moving);
        self.items = rest;
    }

    /// Reordena toda la selección sin cambiar sus tintas.
    pub fn reorder_items(&mut self, ids: &[u64], forward: bool, edge: bool) {
        let movable = |it: &Item| ids.contains(&it.id) && !it.locked;
        let passes = if edge { self.items.len() } else { 1 };
        for _ in 0..passes {
            let mut changed = false;
            let n = self.items.len();
            for k in 0..n.saturating_sub(1) {
                let i = if forward { n - 2 - k } else { k };
                let (a, b) = (&self.items[i], &self.items[i + 1]);
                if if forward { movable(a) && !movable(b) } else { !movable(a) && movable(b) } {
                    self.items.swap(i, i + 1);
                    changed = true;
                }
            }
            if !changed { break; }
        }
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
        // Las fotos de las que ya no queda ninguna tinta no se guardan.
        if self.photos.iter().any(|ph| !self.items.iter().any(|it| it.group == Some(ph.id))) {
            let mut doc = self.clone();
            doc.photos.retain(|ph| self.items.iter().any(|it| it.group == Some(ph.id)));
            return serde_json::to_string_pretty(&doc).unwrap_or_default();
        }
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Registra una foto viva y devuelve su identificador de grupo.
    pub fn add_photo(&mut self, mut photo: crate::photo::Photo) -> u64 {
        photo.id = self.next_id;
        self.next_id += 1;
        let id = photo.id;
        self.photos.push(photo);
        id
    }

    pub fn photo(&self, group: u64) -> Option<&crate::photo::Photo> {
        self.photos.iter().find(|ph| ph.id == group)
    }

    /// Tintas de un grupo, de abajo arriba.
    pub fn group_items(&self, group: u64) -> Vec<u64> {
        self.items.iter().filter(|it| it.group == Some(group)).map(|it| it.id).collect()
    }

    /// Añade los objetos de una importación de foto. Los que traen `group` pasan al grupo
    /// de `photo`. Con `replacing`, sustituyen a las tintas de esa foto en su misma
    /// posición en el papel y en las capas, y la foto anterior desaparece.
    pub fn add_photo_items(&mut self, mut items: Vec<Item>, photo: Option<crate::photo::Photo>, replacing: Option<u64>) -> Vec<u64> {
        let group = photo.map(|ph| self.add_photo(ph));
        for it in &mut items {
            if it.group.is_some() { it.group = group; }
        }
        let old = replacing.map(|g| self.group_items(g)).unwrap_or_default();
        let anchor = old.first().and_then(|id| self.get(*id)).map(|it| it.place);
        let index = old.first().and_then(|id| self.index_of(*id));
        let ids: Vec<u64> = items.into_iter().map(|mut it| {
            if let Some(place) = anchor { it.place = place; }
            self.add(it)
        }).collect();
        if let (Some(index), Some(old_group)) = (index, replacing) {
            let mut fresh: Vec<Item> = Vec::new();
            self.items.retain(|it| {
                if ids.contains(&it.id) { fresh.push(it.clone()); return false; }
                !old.contains(&it.id)
            });
            let index = index.min(self.items.len());
            self.items.splice(index..index, fresh);
            self.photos.retain(|ph| ph.id != old_group);
        }
        ids
    }

    /// Duplica objetos desplazados. Las tintas de una foto duplicada forman un grupo
    /// nuevo con su propia copia de la foto.
    pub fn duplicate(&mut self, ids: &[u64], offset: P) -> Vec<u64> {
        let mut groups: Vec<(u64, u64)> = Vec::new();
        let mut new = Vec::new();
        for id in ids {
            let Some(mut it) = self.get(*id).cloned() else { continue };
            it.place.pos = it.place.pos + offset;
            it.name = format!("{} copia", it.name);
            if let Some(old) = it.group {
                let group = match groups.iter().find(|(o, _)| *o == old) {
                    Some((_, g)) => Some(*g),
                    None => self.photo(old).cloned().map(|mut ph| {
                        ph.name = format!("{} copia", ph.name);
                        let g = self.add_photo(ph);
                        groups.push((old, g));
                        g
                    }),
                };
                it.group = group;
            }
            new.push(self.add(it));
        }
        new
    }

    pub fn from_json(s: &str) -> Result<Document, String> {
        serde_json::from_str(s).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn photo_stub(name: &str) -> crate::photo::Photo {
        crate::photo::Photo { id: 0, name: name.into(), image: crate::Shared::new(crate::photo::EncodedImage { bytes: vec![1, 2, 3] }), params: Default::default(), size_pct: 80.0 }
    }

    fn ink(name: &str) -> Item {
        let mut it = Item::new(name, Geometry::Path { lines: vec![], closed: false }, crate::p(0.0, 0.0));
        it.group = Some(0);
        it
    }

    #[test]
    fn relay_inks_share_the_first_ink_look_and_edit_together() {
        use crate::raster::Grid;
        use crate::trace::{Coverage, TraceParams, Traced};
        let mut doc = Document::default();
        let live = |relay: bool, angle: f64| {
            let cov = Coverage { map: Grid::new(4, 4), mm_per_px: 1.0 };
            let mut it = Item::new("t", Geometry::Traced(Traced::new(cov, TraceParams { relay, angle_deg: angle, ..Default::default() }, 0)), crate::p(0.0, 0.0));
            it.group = Some(0);
            it
        };
        let ids = doc.add_photo_items(vec![live(true, 10.0), live(false, 50.0), live(true, 90.0)], Some(photo_stub("Foto")), None);
        let keys = doc.trace_keys();
        let relay = |id: u64| keys[&id].relay.as_ref().map(|r| (r.index, r.inks.len(), r.look.angle_deg));
        assert_eq!((relay(ids[0]), relay(ids[1]), relay(ids[2])), (Some((0, 2, 10.0)), None, Some((1, 2, 10.0))));
        assert_eq!(doc.with_relay_partners(&[ids[2]]), [ids[0], ids[2]]);
        assert_eq!(doc.with_relay_partners(&[ids[1]]), [ids[1]]);
    }

    #[test]
    fn pass_order_puts_light_inks_first_without_touching_other_pens() {
        let pens = vec![Pen::new("Otra", [0, 200, 0], 0.5), Pen::new("Negro", [20, 20, 20], 0.5), Pen::new("Amarillo", [250, 220, 30], 0.5), Pen::new("Rojo", [210, 40, 40], 0.5)];
        let mut doc = Document { pens, ..Default::default() };
        use crate::geom::p;
        let ink = |doc: &mut Document, pen: usize| {
            let mut it = Item::new("t", Geometry::Path { lines: vec![vec![p(0.0, 0.0), p(1.0, 0.0)]], closed: false }, p(0.0, 0.0));
            it.stroke = Some(pen);
            doc.add(it)
        };
        let other = ink(&mut doc, 0);
        let ids = [ink(&mut doc, 1), ink(&mut doc, 2), ink(&mut doc, 3)];
        doc.order_passes(&ids, true);
        let names = |doc: &Document| doc.pens.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&doc), ["Otra", "Amarillo", "Rojo", "Negro"]);
        let color = |doc: &Document, id| doc.pens[doc.get(id).unwrap().stroke.unwrap()].name.clone();
        assert_eq!((color(&doc, ids[0]), color(&doc, ids[1]), color(&doc, other)), ("Negro".into(), "Amarillo".into(), "Otra".into()));
        assert_eq!(doc.items.iter().map(|it| color(&doc, it.id)).collect::<Vec<_>>(), ["Otra", "Amarillo", "Rojo", "Negro"]);
        doc.order_passes(&ids, false);
        assert_eq!(names(&doc), ["Otra", "Negro", "Rojo", "Amarillo"]);
    }

    #[test]
    fn photo_groups_replace_in_place_duplicate_apart_and_prune_on_save() {
        let mut d = Document::default();
        let below = d.add(line(0));
        let first = d.add_photo_items(vec![ink("A"), ink("B")], Some(photo_stub("Retrato")), None);
        let above = d.add(line(1));
        let group = d.get(first[0]).unwrap().group.unwrap();
        assert_eq!(d.group_items(group), first);
        d.get_mut(first[0]).unwrap().place.pos = crate::p(50.0, 60.0);

        // Volver a separar: las tintas nuevas ocupan el sitio de las anteriores.
        let second = d.add_photo_items(vec![ink("C"), ink("D"), ink("E")], Some(photo_stub("Retrato")), Some(group));
        let order: Vec<u64> = d.items.iter().map(|it| it.id).collect();
        assert_eq!(order, [vec![below], second.clone(), vec![above]].concat());
        assert!(d.photo(group).is_none() && d.photos.len() == 1);
        assert!(second.iter().all(|id| d.get(*id).unwrap().place.pos == crate::p(50.0, 60.0)));

        // Duplicar crea un grupo nuevo con su propia foto.
        let new_group = d.get(second[0]).unwrap().group.unwrap();
        let copies = d.duplicate(&second[..2], crate::p(8.0, 8.0));
        let copy_group = d.get(copies[0]).unwrap().group.unwrap();
        assert_ne!(copy_group, new_group);
        assert_eq!(d.group_items(copy_group), copies);
        assert_eq!(d.photo(copy_group).unwrap().name, "Retrato copia");

        // Una foto sin tintas no se guarda.
        d.items.retain(|it| it.group != Some(copy_group));
        let saved = Document::from_json(&d.to_json()).unwrap();
        assert_eq!(saved.photos.iter().map(|ph| ph.id).collect::<Vec<_>>(), vec![new_group]);
        assert_eq!(saved.photos[0].image.bytes, vec![1, 2, 3]);
    }

    #[test]
    fn empty_text_frame_can_be_selected_and_has_exact_bounds() {
        let it = Item::new("Texto", Geometry::Text(TextSpec { text: String::new(), wrap_mm: 180.0, frame_height_mm: 267.0, ..Default::default() }), crate::p(15.0, 15.0));
        assert_eq!(it.bbox().width(), 180.0);
        assert_eq!(it.bbox().height(), 267.0);
        assert!(it.hit(crate::p(100.0, 100.0), 0.0));
        assert!(!it.hit(crate::p(200.0, 100.0), 0.0));
        assert!(it.plot(&default_pens()).iter().all(|(_, lines)| lines.is_empty()));
    }

    fn line(pen: usize) -> Item {
        let mut it = Item::new(format!("p{pen}"), Geometry::Path { lines: vec![vec![crate::geom::p(0.0, 0.0), crate::geom::p(1.0, 1.0)]], closed: false }, P::default());
        it.stroke = Some(pen);
        it
    }

    #[test]
    fn automatic_ink_names_follow_color_and_custom_names_are_preserved() {
        let mut automatic = default_pens().remove(0);
        automatic.set_color([150, 60, 180]);
        assert_eq!(automatic.name, "Morado");
        assert_eq!(automatic.width_mm, 0.4);
        automatic.name = "Rotulador de Raúl".into();
        automatic.custom_name = true;
        automatic.set_color([20, 140, 60]);
        assert_eq!(automatic.name, "Rotulador de Raúl");
        let mut legacy_label = Pen::new("Edición especial", [1, 2, 3], 0.8);
        legacy_label.set_color([200, 30, 40]);
        assert_eq!(legacy_label.name, "Edición especial");
    }

    #[test]
    fn moving_selection_preserves_inks_order_and_locked_objects() {
        let mut d = Document::default();
        let a = d.add(line(0));
        let b = d.add(line(1));
        let locked = d.add(line(2));
        d.get_mut(locked).unwrap().locked = true;
        let target = d.add(line(3));
        let originals = d.items.clone();
        d.move_items(&[b, locked, a], Some((target, true)));
        assert_eq!(d.items.iter().map(|it| it.id).collect::<Vec<_>>(), vec![locked, target, a, b]);
        for it in &originals { assert_eq!(d.get(it.id), Some(it)); }
    }

    #[test]
    fn editing_object_ink_preserves_other_objects_and_reuses_exclusive_ink() {
        let mut d = Document::default();
        let a = d.add(line(1));
        let b = d.add(line(1));
        let width = d.pens[1].width_mm;
        let index = d.isolate_ink(a, false).unwrap();
        assert_ne!(index, 1);
        d.pens[index].color = [12, 34, 56];
        assert_eq!(d.get(b).unwrap().stroke, Some(1));
        assert_eq!(d.pens[1].color, [214, 48, 49]);
        assert_eq!(d.pens[index].width_mm, width);
        let count = d.pens.len();
        assert_eq!(d.isolate_ink(a, false), Some(index));
        assert_eq!(d.pens.len(), count);
    }

    #[test]
    fn editing_fill_color_keeps_outline_and_locked_objects_unchanged() {
        let mut d = Document::default();
        let mut it = Item::new("Figura", Geometry::Shape(Shape::Heart { size: 30.0 }), P::default());
        it.stroke = Some(1);
        it.fill.pen = 1;
        it.fill.style = fill::FillStyle::Hatch;
        let id = d.add(it);
        let index = d.isolate_ink(id, true).unwrap();
        assert_ne!(index, 1);
        d.pens[index].color = [12, 34, 56];
        assert_eq!(d.get(id).unwrap().stroke, Some(1));
        assert_eq!(d.pens[1].color, [214, 48, 49]);
        d.get_mut(id).unwrap().locked = true;
        let before = d.clone();
        assert_eq!(d.isolate_ink(id, false), None);
        assert_eq!(d, before);
    }

    #[test]
    fn screened_fill_keeps_its_ink_and_paths_when_saved_and_recolored() {
        let mut d = Document::default();
        let lines = vec![vec![crate::p(0.0, 0.0), crate::p(5.0, 0.0)]];
        let mut it = Item::new("Mezcla", Geometry::Screened { lines: lines.clone(), screen: crate::mix::Screen::Hatch }, crate::p(20.0, 30.0));
        it.stroke = None;
        it.fill.pen = 1;
        let id = d.add(it);
        let other = d.add(line(1));
        let mut restored = Document::from_json(&d.to_json()).unwrap();
        assert_eq!(restored, d);
        let index = restored.isolate_ink(id, true).unwrap();
        assert_ne!(index, 1);
        restored.pens[index].color = [1, 2, 3];
        let it = restored.get(id).unwrap();
        assert!(it.has_fill());
        assert_eq!(it.stroke, None);
        assert_eq!(it.plot(&restored.pens)[0], (index, vec![vec![crate::p(20.0, 30.0), crate::p(25.0, 30.0)]]));
        assert_eq!(restored.get(other).unwrap().stroke, Some(1));
        assert_eq!(restored.isolate_ink(id, false), None);
    }

    #[test]
    fn dropping_a_palette_ink_assigns_outline_or_fill_and_respects_locks() {
        let mut d = Document::default();
        let a = d.add(line(0));
        let b = d.add(line(0));
        assert!(d.assign_pen(a, 2, false));
        assert_eq!(d.get(a).unwrap().stroke, Some(2));
        assert_eq!(d.get(b).unwrap().stroke, Some(0));
        // Un trazo abierto no tiene relleno que recibir.
        assert!(!d.assign_pen(a, 3, true));
        assert!(!d.assign_pen(a, 99, false));
        let shape = d.add(Item::new("Figura", Geometry::Shape(Shape::Heart { size: 30.0 }), P::default()));
        assert!(d.assign_pen(shape, 3, true));
        let it = d.get(shape).unwrap();
        assert_eq!((it.fill.pen, it.fill.style), (3, fill::FillStyle::Hatch));
        let mut screened = Item::new("Mezcla", Geometry::Screened { lines: vec![], screen: crate::mix::Screen::Hatch }, P::default());
        screened.stroke = None;
        let screened = d.add(screened);
        assert!(d.assign_pen(screened, 1, false));
        assert_eq!((d.get(screened).unwrap().fill.pen, d.get(screened).unwrap().stroke), (1, None));
        d.get_mut(b).unwrap().locked = true;
        let before = d.clone();
        assert!(!d.assign_pen(b, 2, false));
        assert_eq!(d, before);
    }

    #[test]
    fn dropping_on_selection_does_not_change_document() {
        let mut d = Document::default();
        let id = d.add(line(0));
        let before = d.clone();
        d.move_items(&[id], Some((id, true)));
        assert_eq!(d, before);
    }

    #[test]
    fn reorder_selection_freely_across_colors_without_changing_inks() {
        let mut d = Document::default();
        let a = d.add(line(0));
        let b = d.add(line(1));
        let c = d.add(line(2));
        let other = d.add(line(3));
        let originals = d.items.clone();
        d.reorder_items(&[a, b], true, false);
        assert_eq!(d.items.iter().map(|it| it.id).collect::<Vec<_>>(), vec![c, a, b, other]);
        d.reorder_items(&[a, b], true, true);
        assert_eq!(d.items.iter().map(|it| it.id).collect::<Vec<_>>(), vec![c, other, a, b]);
        d.reorder_items(&[a, b], false, true);
        assert_eq!(d.items.iter().map(|it| it.id).collect::<Vec<_>>(), vec![a, b, c, other]);
        for it in &originals { assert_eq!(d.get(it.id), Some(it)); }
        d.get_mut(a).unwrap().locked = true;
        let before = d.clone();
        d.reorder_items(&[a], true, true);
        assert_eq!(d, before);
    }

    #[test]
    fn reordering_palette_keeps_object_order_and_colors() {
        let mut d = Document::default();
        d.pens.truncate(3);
        for pen in [2, 0, 1] { d.add(line(pen)); }
        let before: Vec<_> = d.items.iter().map(|it| (it.id, d.pens[it.stroke.unwrap()].color)).collect();
        assert_eq!(d.move_pen(0, 2), vec![2, 0, 1]);
        let after: Vec<_> = d.items.iter().map(|it| (it.id, d.pens[it.stroke.unwrap()].color)).collect();
        assert_eq!(before, after);
    }
}
