//! Selección directa de polilíneas fijas en sus coordenadas locales.
use std::collections::BTreeSet;
use vectorio_core::{Document, Geometry, Item, Polyline, P};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct StrokeId(pub u64, pub usize);

#[derive(Default)]
pub(crate) struct Selection {
    pub ids: BTreeSet<StrokeId>,
    pub drag_start: Option<P>,
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

pub(crate) fn nearest(doc: &Document, point: P, tolerance: f64) -> Option<StrokeId> {
    let mut hit = None;
    let mut best = f64::INFINITY;
    for item in doc.items.iter().rev() {
        let Some(paths) = lines(item) else { continue };
        let pen = if item.is_screened() {
            item.fill.pen
        } else {
            item.stroke?
        };
        let radius = tolerance + doc.pens.get(pen).map_or(0.0, |p| p.width_mm / 2.0);
        let q = item.place.inverse(point);
        for (index, line) in paths.iter().enumerate() {
            let distance = vectorio_core::geom::dist_to_lines(q, std::slice::from_ref(line))
                * item.place.scale;
            if distance <= radius && distance < best {
                best = distance;
                hit = Some(StrokeId(item.id, index));
                if distance == 0.0 {
                    return hit;
                }
            }
        }
    }
    hit
}

impl Selection {
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

    pub fn select_all(&mut self, doc: &Document) {
        self.ids.clear();
        for item in &doc.items {
            if let Some(lines) = lines(item) {
                self.ids
                    .extend((0..lines.len()).map(|i| StrokeId(item.id, i)));
            }
        }
    }

    pub fn move_by(&self, doc: &mut Document, delta: P) {
        for item in &mut doc.items {
            if !editable(item) {
                continue;
            }
            let local = item.place.inverse(item.place.pos + delta);
            let (Geometry::Path { lines, .. } | Geometry::Screened { lines, .. }) =
                &mut item.geometry
            else {
                continue;
            };
            for id in self
                .ids
                .range(StrokeId(item.id, 0)..=StrokeId(item.id, usize::MAX))
            {
                if let Some(line) = lines.get_mut(id.1) {
                    for point in line {
                        *point = *point + local;
                    }
                }
            }
        }
    }

    pub fn delete(&mut self, doc: &mut Document) {
        doc.items.retain_mut(|item| {
            if !editable(item)
                || self
                    .ids
                    .range(StrokeId(item.id, 0)..=StrokeId(item.id, usize::MAX))
                    .next()
                    .is_none()
            {
                return true;
            }
            let (Geometry::Path { lines, .. } | Geometry::Screened { lines, .. }) =
                &mut item.geometry
            else {
                return true;
            };
            let mut index = 0;
            lines.retain(|_| {
                let keep = !self.ids.contains(&StrokeId(item.id, index));
                index += 1;
                keep
            });
            !lines.is_empty()
        });
        *self = Self::default();
    }
}
