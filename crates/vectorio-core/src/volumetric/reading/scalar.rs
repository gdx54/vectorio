//! Isolíneas del campo por regiones visibles; interpolación en el espacio del modelo.
use super::*;
use std::collections::BTreeMap;
pub(super) fn join(edges: Vec<[P; 2]>, cancel: &dyn Fn() -> bool) -> Result<Vec<Polyline>, String> {
    let key = |q: P| ((q.x * 1e6).round() as i64, (q.y * 1e6).round() as i64);
    let mut nodes = BTreeMap::new();
    let mut points = Vec::new();
    let mut links: Vec<Vec<usize>> = Vec::new();
    let mut segments = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for edge in edges {
        let mut ids = [0; 2];
        for i in 0..2 {
            let next = nodes.len();
            let id = *nodes.entry(key(edge[i])).or_insert(next);
            if id == points.len() {
                points.push(edge[i]);
                links.push(vec![]);
            }
            ids[i] = id;
        }
        if ids[0] == ids[1] || !seen.insert((ids[0].min(ids[1]), ids[0].max(ids[1]))) {
            continue;
        }
        let id = segments.len();
        for n in ids {
            links[n].push(id);
        }
        segments.push(ids);
    }
    let mut used = vec![false; segments.len()];
    let mut out = Vec::new();
    let mut starts = (0..points.len())
        .filter(|i| links[*i].len() != 2)
        .collect::<Vec<_>>();
    starts.extend(0..points.len());
    for start in starts {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        for &edge in &links[start] {
            if used[edge] {
                continue;
            }
            let mut at = start;
            let mut id = edge;
            let mut line = vec![points[start]];
            loop {
                used[id] = true;
                let e = segments[id];
                at = if e[0] == at { e[1] } else { e[0] };
                line.push(points[at]);
                if links[at].len() != 2 {
                    break;
                }
                let Some(next) = links[at].iter().copied().find(|i| !used[*i]) else {
                    break;
                };
                id = next;
            }
            if line.len() > 1 {
                out.push(line);
            }
        }
    }
    Ok(out)
}
pub(super) fn draw(
    surface: &surface::Surface,
    d: &Drawing,
    values: &fields::Values,
    geometry: &fields::Geometry,
    s: &Settings,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Polyline>>, String> {
    let map = &values.config;
    let mut levels = if let Some(t) = map.threshold {
        vec![t]
    } else {
        (1..=map.levels)
            .map(|i| values.raw_at(i as f64 / (map.levels + 1) as f64))
            .collect::<Vec<_>>()
    };
    levels.sort_by(f64::total_cmp);
    levels.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
    let segments = surface.isolines(&levels, values, geometry, &d.mesh, s, cancel)?;
    let mut out = vec![vec![]; map.pens.len()];
    let mut labels = Vec::<BBox>::new();
    for (level, segments) in levels.into_iter().zip(segments) {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let Some(slot) = encode::slot(map, values.normalize(level), 0) else {
            continue;
        };
        let curves = join(segments, cancel)?;
        if map.label_values {
            if let Some(line) = curves
                .iter()
                .max_by(|a, b| crate::geom::length(a).total_cmp(&crate::geom::length(b)))
                .filter(|l| crate::geom::length(l) > 10.)
            {
                let q = line[line.len() / 2] + p(0., -2.);
                let mut text = vec![];
                legend::text(&mut text, &format!("{level:.2}"), q, 1.5, 16.);
                let bounds = BBox::of(&text);
                if !labels.iter().any(|b| {
                    bounds.min.x <= b.max.x + 2.
                        && bounds.max.x + 2. >= b.min.x
                        && bounds.min.y <= b.max.y + 2.
                        && bounds.max.y + 2. >= b.min.y
                }) {
                    labels.push(bounds);
                    out[slot].extend(text);
                }
            }
        }
        out[slot].extend(curves);
    }
    Ok(out)
}
