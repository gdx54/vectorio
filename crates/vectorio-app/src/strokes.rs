//! Selección directa de polilíneas fijas en sus coordenadas locales.
use std::collections::{BTreeSet, HashMap};
use vectorio_core::{BBox, Document, Geometry, Item, Polyline, P};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct StrokeId(pub u64, pub usize);

#[derive(Default)]
pub(crate) struct Selection {
    pub ids: BTreeSet<StrokeId>,
    pub drag_start: Option<P>,
    /// Esquina donde empezó el recuadro de selección (mm del papel).
    pub marquee: Option<P>,
    /// Foto viva que se pulsó: sus líneas no se pueden tocar hasta fijarlas.
    pub live: Option<u64>,
    pub index: Index,
}

fn editable(item: &Item) -> bool {
    item.visible
        && !item.locked
        && match &item.geometry {
            Geometry::Path { .. } => item.stroke.is_some() && !item.has_fill(),
            Geometry::Screened { .. } => true,
            _ => false,
        }
}

pub(crate) fn lines(item: &Item) -> Option<&[Polyline]> {
    if !editable(item) {
        return None;
    }
    match &item.geometry {
        Geometry::Path { lines, .. } | Geometry::Screened { lines, .. } => Some(lines),
        _ => None,
    }
}

fn lines_mut(item: &mut Item) -> Option<&mut Vec<Polyline>> {
    if !editable(item) {
        return None;
    }
    match &mut item.geometry {
        Geometry::Path { lines, .. } | Geometry::Screened { lines, .. } => Some(lines),
        _ => None,
    }
}

fn pen_of(item: &Item) -> Option<usize> {
    if item.is_screened() { Some(item.fill.pen) } else { item.stroke }
}

/// Foto viva visible y desbloqueada bajo el punto, si la hay.
pub(crate) fn live_photo_at(doc: &Document, point: P, tolerance: f64) -> Option<u64> {
    doc.items
        .iter()
        .rev()
        .find(|it| it.visible && !it.locked && it.traced().is_some() && it.hit(point, tolerance))
        .map(|it| it.id)
}

/// Convierte en trazos fijos las tintas vivas de la foto de `id` (o solo ese objeto si
/// no pertenece a ninguna). Devuelve cuántas se han fijado.
pub(crate) fn freeze_live(doc: &mut Document, id: u64) -> usize {
    let members = match doc.get(id).and_then(|it| it.group) {
        Some(group) => doc.group_items(group),
        None => vec![id],
    };
    let mut frozen = 0;
    for member in members {
        if let Some(it) = doc.get_mut(member).filter(|it| !it.locked) {
            if let Geometry::Traced(t) = &it.geometry {
                it.geometry = Geometry::Screened { lines: t.lines.to_vec(), screen: t.trace.style };
                frozen += 1;
            }
        }
    }
    frozen
}

// ── Índice por zonas ─────────────────────────────────────────────────────────────

/// Lado de cada casilla de la rejilla, en mm del papel.
const CELL: f64 = 4.0;

/// Rejilla con la caja de cada línea editable, en coordenadas del papel. Evita recorrer
/// miles de líneas de una foto en cada movimiento del ratón. Se reconstruye cuando cambia
/// algún objeto (otra geometría, colocación o visibilidad) o tras editar sus líneas.
#[derive(Default)]
pub(crate) struct Index {
    key: Vec<(u64, usize, usize, [u64; 4])>,
    boxes: Vec<(StrokeId, BBox)>,
    cells: HashMap<(i64, i64), Vec<u32>>,
    built: bool,
}

fn cell(v: f64) -> i64 {
    (v / CELL).floor() as i64
}

fn fingerprint(doc: &Document) -> Vec<(u64, usize, usize, [u64; 4])> {
    doc.items
        .iter()
        .filter_map(|it| {
            let lines = lines(it)?;
            let place = [it.place.pos.x.to_bits(), it.place.pos.y.to_bits(), it.place.scale.to_bits(), it.place.rotation_deg.to_bits()];
            Some((it.id, lines.as_ptr() as usize, lines.len(), place))
        })
        .collect()
}

impl Index {
    pub fn invalidate(&mut self) {
        self.built = false;
    }

    pub fn refresh(&mut self, doc: &Document) {
        let key = fingerprint(doc);
        if self.built && key == self.key {
            return;
        }
        self.key = key;
        self.boxes.clear();
        self.cells.clear();
        for item in &doc.items {
            let Some(paths) = lines(item) else { continue };
            for (index, line) in paths.iter().enumerate() {
                let mut bbox = BBox::empty();
                for q in line {
                    bbox.add(item.place.apply(*q));
                }
                if bbox.is_empty() {
                    continue;
                }
                let slot = self.boxes.len() as u32;
                self.boxes.push((StrokeId(item.id, index), bbox));
                for cx in cell(bbox.min.x)..=cell(bbox.max.x) {
                    for cy in cell(bbox.min.y)..=cell(bbox.max.y) {
                        self.cells.entry((cx, cy)).or_default().push(slot);
                    }
                }
            }
        }
        self.built = true;
    }

    /// Líneas cuya caja toca el rectángulo (mm del papel), sin repetir.
    fn candidates(&self, min: P, max: P) -> Vec<&(StrokeId, BBox)> {
        let mut seen = BTreeSet::new();
        for cx in cell(min.x)..=cell(max.x) {
            for cy in cell(min.y)..=cell(max.y) {
                if let Some(slots) = self.cells.get(&(cx, cy)) {
                    seen.extend(slots.iter().copied());
                }
            }
        }
        seen.into_iter()
            .map(|slot| &self.boxes[slot as usize])
            .filter(|(_, b)| b.min.x <= max.x && b.max.x >= min.x && b.min.y <= max.y && b.max.y >= min.y)
            .collect()
    }

    /// Línea más cercana al punto, dentro de la tolerancia más medio grosor de su tinta.
    /// Entre varias igual de cerca gana la del objeto de encima.
    pub fn nearest(&self, doc: &Document, point: P, tolerance: f64) -> Option<StrokeId> {
        let widest = doc.pens.iter().map(|p| p.width_mm / 2.0).fold(0.0, f64::max);
        let reach = tolerance + widest;
        let order: HashMap<u64, usize> = doc.items.iter().enumerate().map(|(i, it)| (it.id, i)).collect();
        let mut best: Option<(f64, usize, StrokeId)> = None;
        for (id, _) in self.candidates(point - vectorio_core::p(reach, reach), point + vectorio_core::p(reach, reach)) {
            let Some(item) = doc.get(id.0) else { continue };
            let (Some(paths), Some(pen)) = (lines(item), pen_of(item)) else { continue };
            let Some(line) = paths.get(id.1) else { continue };
            let radius = tolerance + doc.pens.get(pen).map_or(0.0, |p| p.width_mm / 2.0);
            let distance = vectorio_core::geom::dist_to_lines(item.place.inverse(point), std::slice::from_ref(line)) * item.place.scale;
            let depth = order.get(&id.0).copied().unwrap_or(0);
            let better = best.is_none_or(|(d, z, _)| distance < d - 1e-9 || (distance <= d + 1e-9 && depth > z));
            if distance <= radius && better {
                best = Some((distance, depth, *id));
            }
        }
        best.map(|(_, _, id)| id)
    }

    /// Líneas que tocan el rectángulo entre `a` y `b` (mm del papel).
    pub fn in_rect(&self, doc: &Document, a: P, b: P) -> Vec<StrokeId> {
        let min = vectorio_core::p(a.x.min(b.x), a.y.min(b.y));
        let max = vectorio_core::p(a.x.max(b.x), a.y.max(b.y));
        let mut found = Vec::new();
        for (id, bbox) in self.candidates(min, max) {
            let inside = bbox.min.x >= min.x && bbox.max.x <= max.x && bbox.min.y >= min.y && bbox.max.y <= max.y;
            let touches = inside
                || doc.get(id.0).and_then(|item| Some((item, lines(item)?.get(id.1)?))).is_some_and(|(item, line)| {
                    let world: Vec<P> = line.iter().map(|q| item.place.apply(*q)).collect();
                    world.iter().any(|q| q.x >= min.x && q.x <= max.x && q.y >= min.y && q.y <= max.y)
                        || world.windows(2).any(|w| segment_hits_rect(w[0], w[1], min, max))
                });
            if touches {
                found.push(*id);
            }
        }
        found
    }
}

/// ¿Cruza el segmento el rectángulo? (Liang–Barsky).
fn segment_hits_rect(a: P, b: P, min: P, max: P) -> bool {
    let d = b - a;
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (p, q) in [(-d.x, a.x - min.x), (d.x, max.x - a.x), (-d.y, a.y - min.y), (d.y, max.y - a.y)] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let r = q / p;
            if p < 0.0 { t0 = t0.max(r) } else { t1 = t1.min(r) }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

// ── Selección ────────────────────────────────────────────────────────────────────

/// Cómo combina un recuadro con la selección que ya había.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Combine {
    Replace,
    Add,
    Remove,
}

impl Selection {
    fn of_item(&self, item: u64) -> impl Iterator<Item = &StrokeId> {
        self.ids.range(StrokeId(item, 0)..=StrokeId(item, usize::MAX))
    }

    pub fn pick(&mut self, hit: Option<StrokeId>, extend: bool) {
        if !extend {
            self.ids.clear();
        }
        if let Some(id) = hit {
            if !self.ids.insert(id) && extend {
                self.ids.remove(&id);
            }
        }
    }

    pub fn combine(&mut self, found: impl IntoIterator<Item = StrokeId>, how: Combine) {
        if how == Combine::Replace {
            self.ids.clear();
        }
        for id in found {
            if how == Combine::Remove { self.ids.remove(&id); } else { self.ids.insert(id); }
        }
    }

    pub fn select_all(&mut self, doc: &Document) {
        self.ids.clear();
        for item in &doc.items {
            if let Some(lines) = lines(item) {
                self.ids.extend((0..lines.len()).map(|i| StrokeId(item.id, i)));
            }
        }
    }

    pub fn move_by(&mut self, doc: &mut Document, delta: P) {
        for item in &mut doc.items {
            let local = item.place.inverse(item.place.pos + delta);
            let picked: Vec<usize> = self.of_item(item.id).map(|id| id.1).collect();
            let Some(lines) = lines_mut(item) else { continue };
            for index in picked {
                if let Some(line) = lines.get_mut(index) {
                    for point in line {
                        *point = *point + local;
                    }
                }
            }
        }
        self.index.invalidate();
    }

    pub fn delete(&mut self, doc: &mut Document) {
        let ids = std::mem::take(&mut self.ids);
        doc.items.retain_mut(|item| {
            let id = item.id;
            let Some(lines) = lines_mut(item) else { return true };
            if ids.range(StrokeId(id, 0)..=StrokeId(id, usize::MAX)).next().is_none() {
                return true;
            }
            let mut index = 0;
            lines.retain(|_| {
                let keep = !ids.contains(&StrokeId(id, index));
                index += 1;
                keep
            });
            !lines.is_empty()
        });
        self.drag_start = None;
        self.index.invalidate();
    }

    /// Saca las líneas elegidas de cada objeto a un objeto nuevo, justo encima del
    /// original y con su misma tinta, colocación y tipo de trazo. Si se eligen todas las
    /// líneas de un objeto, ese objeto ya está separado y se deja como está. Devuelve los
    /// objetos nuevos.
    pub fn separate(&mut self, doc: &mut Document, name: impl Fn(&str) -> String) -> Vec<u64> {
        let mut created = Vec::new();
        let sources: Vec<u64> = self.ids.iter().map(|id| id.0).collect::<BTreeSet<_>>().into_iter().collect();
        for source in sources {
            let picked: BTreeSet<usize> = self.of_item(source).map(|id| id.1).collect();
            let Some(position) = doc.index_of(source) else { continue };
            let item = &mut doc.items[position];
            let Some(lines) = lines_mut(item) else { continue };
            let picked: BTreeSet<usize> = picked.into_iter().filter(|i| *i < lines.len()).collect();
            if picked.is_empty() || picked.len() == lines.len() {
                continue;
            }
            let mut taken = Vec::with_capacity(picked.len());
            let mut kept = Vec::with_capacity(lines.len() - picked.len());
            for (i, line) in std::mem::take(lines).into_iter().enumerate() {
                if picked.contains(&i) { taken.push(line) } else { kept.push(line) }
            }
            *lines = kept;
            let mut part = item.clone();
            part.name = name(&item.name);
            part.group = None;
            match &mut part.geometry {
                Geometry::Path { lines, .. } | Geometry::Screened { lines, .. } => *lines = taken,
                _ => unreachable!(),
            }
            let id = doc.add(part);
            let new = doc.items.pop().expect("recién añadido");
            doc.items.insert(position + 1, new);
            created.push(id);
        }
        if !created.is_empty() {
            self.ids.clear();
            self.index.invalidate();
        }
        created
    }

    /// ¿Hay algún objeto del que se haya elegido solo una parte de sus líneas?
    pub fn can_separate(&self, doc: &Document) -> bool {
        let sources: BTreeSet<u64> = self.ids.iter().map(|id| id.0).collect();
        sources.into_iter().any(|source| {
            doc.get(source).and_then(lines).is_some_and(|lines| self.of_item(source).count() < lines.len())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorio_core::p;

    fn grid(n: usize) -> Document {
        let mut doc = Document::default();
        let lines: Vec<Polyline> = (0..n).map(|i| vec![p(0.0, i as f64), p(10.0, i as f64)]).collect();
        let mut item = Item::new("Foto", Geometry::Path { lines, closed: false }, p(50.0, 50.0));
        item.group = Some(99);
        doc.add(item);
        doc
    }

    #[test]
    fn the_index_finds_the_same_line_as_scanning_and_follows_moves() {
        let mut doc = grid(200);
        let mut sel = Selection::default();
        sel.index.refresh(&doc);
        assert_eq!(sel.index.nearest(&doc, p(55.0, 120.1), 0.4), Some(StrokeId(1, 70)));
        assert_eq!(sel.index.nearest(&doc, p(75.0, 120.0), 0.4), None);
        sel.pick(Some(StrokeId(1, 70)), false);
        sel.move_by(&mut doc, p(30.0, 0.0));
        sel.index.refresh(&doc);
        assert_eq!(sel.index.nearest(&doc, p(85.0, 120.0), 0.4), Some(StrokeId(1, 70)));
        // Mover el objeto entero también reconstruye el índice.
        doc.items[0].place.pos = p(0.0, 0.0);
        sel.index.refresh(&doc);
        assert_eq!(sel.index.nearest(&doc, p(5.0, 3.0), 0.4), Some(StrokeId(1, 3)));
    }

    #[test]
    fn a_box_selects_the_lines_it_touches_and_combines_with_the_selection() {
        let doc = grid(20);
        let mut sel = Selection::default();
        sel.index.refresh(&doc);
        // Un recuadro estrecho en medio cruza las líneas 4..=6 aunque no contiene ningún extremo.
        let found = sel.index.in_rect(&doc, p(54.0, 53.5), p(56.0, 56.5));
        assert_eq!(found, vec![StrokeId(1, 4), StrokeId(1, 5), StrokeId(1, 6)]);
        sel.combine(found, Combine::Replace);
        sel.combine(sel.index.in_rect(&doc, p(40.0, 59.5), p(70.0, 60.5)), Combine::Add);
        sel.combine([StrokeId(1, 5)], Combine::Remove);
        assert_eq!(sel.ids.iter().map(|id| id.1).collect::<Vec<_>>(), vec![4, 6, 10]);
    }

    #[test]
    fn separating_moves_the_chosen_lines_to_a_new_object_above_with_the_same_ink() {
        let mut doc = grid(5);
        doc.items[0].stroke = Some(2);
        doc.add(Item::new("Encima", Geometry::Path { lines: vec![vec![p(0.0, 0.0), p(1.0, 0.0)]], closed: false }, p(0.0, 0.0)));
        let mut sel = Selection::default();
        sel.combine([StrokeId(1, 1), StrokeId(1, 3)], Combine::Replace);
        assert!(sel.can_separate(&doc));
        let created = sel.separate(&mut doc, |n| format!("{n} · parte"));
        assert_eq!(created.len(), 1);
        let names: Vec<&str> = doc.items.iter().map(|it| it.name.as_str()).collect();
        assert_eq!(names, ["Foto", "Foto · parte", "Encima"]);
        let part = doc.get(created[0]).unwrap();
        assert_eq!((part.stroke, part.group, part.place), (Some(2), None, doc.items[0].place));
        assert_eq!(part.local_lines().0, vec![vec![p(0.0, 1.0), p(10.0, 1.0)], vec![p(0.0, 3.0), p(10.0, 3.0)]]);
        assert_eq!(doc.items[0].local_lines().0.len(), 3);
        assert!(sel.ids.is_empty());
        // Elegir todas las líneas de un objeto no crea nada: ya es un objeto propio.
        sel.select_all(&doc);
        sel.ids.retain(|id| id.0 == created[0]);
        assert!(!sel.can_separate(&doc));
        assert!(sel.separate(&mut doc, |n| n.to_string()).is_empty());
    }

    #[test]
    fn segments_and_rectangles() {
        let (min, max) = (p(0.0, 0.0), p(1.0, 1.0));
        assert!(segment_hits_rect(p(-1.0, 0.5), p(2.0, 0.5), min, max));
        assert!(segment_hits_rect(p(-1.0, -1.0), p(2.0, 2.0), min, max));
        assert!(!segment_hits_rect(p(-1.0, 2.0), p(2.0, 2.5), min, max));
        assert!(!segment_hits_rect(p(1.5, -1.0), p(3.0, 2.0), min, max));
    }
}
