//! Pasadas de lectura: hachuras, agujas, horizonte analítico y vistas múltiples.
use super::surface::Surface;
use super::*;
pub(super) fn area(paper: &crate::Paper) -> BBox {
    BBox {
        min: p(paper.margin_mm, paper.margin_mm),
        max: p(
            paper.width_mm - paper.margin_mm,
            paper.height_mm - paper.margin_mm,
        ),
    }
}
pub(super) fn grid(
    area: BBox,
    spacing: f64,
    angle: f64,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Polyline>, String> {
    let (s, c) = angle.to_radians().sin_cos();
    let along = p(c, s);
    let normal = p(-s, c);
    let corners = [
        area.min,
        p(area.max.x, area.min.y),
        area.max,
        p(area.min.x, area.max.y),
    ];
    let (lo, hi) = corners
        .iter()
        .map(|q| q.dot(normal))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
    let size = area.width() + area.height();
    let mut lines = Vec::new();
    for i in (lo / spacing).ceil() as i64..=(hi / spacing).floor() as i64 {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let center = normal * (i as f64 * spacing);
        lines.push(vec![center - along * size, center + along * size]);
    }
    Ok(encode::clip(&lines, area))
}
pub(super) fn marks(
    d: &Drawing,
    s: &Settings,
    map: &ReadingMap,
    effect: &ReadingEffect,
    values: &fields::Values,
    geometry: &fields::Geometry,
    surface: &Surface,
    paper: &crate::Paper,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Polyline>, String> {
    let bounds = area(paper);
    let step = map.spacing_mm.max(0.5) * 2.;
    let mut lines = Vec::new();
    for row in (bounds.min.y / step).ceil() as i64..=(bounds.max.y / step).floor() as i64 {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        for col in (bounds.min.x / step).ceil() as i64..=(bounds.max.x / step).floor() as i64 {
            let point = p(col as f64 * step, row as f64 * step);
            let Some(sample) = surface.at(point, values, geometry, &d.mesh) else {
                continue;
            };
            let (vector, size) = match *effect {
                ReadingEffect::Relief { length_mm } => {
                    let slope = (1. - sample.normal.z * sample.normal.z).max(0.).sqrt();
                    if encode::rank((row * 71 + col).unsigned_abs() as usize) > slope {
                        continue;
                    }
                    (
                        V3::new(0., 0., -1.) + sample.normal * sample.normal.z,
                        length_mm * slope,
                    )
                }
                ReadingEffect::Needles { size_mm } => (sample.normal, size_mm),
                _ => unreachable!(),
            };
            let a = d.camera.project(sample.world).0;
            let b = d.camera.project(sample.world + vector).0;
            let delta = (b - a).rotate(s.paper_rotation.to_radians());
            let magnitude = (delta.len() / d.camera.scale).min(1.);
            if delta.len() < 1e-8 {
                continue;
            }
            let delta = delta * (size * magnitude / delta.len());
            if delta.len() < 0.1 {
                continue;
            }
            lines.push(vec![point - delta * 0.5, point + delta * 0.5]);
            if lines.len() > 120_000 {
                return Err("Demasiadas hachuras; aumenta su separación.".into());
            }
        }
    }
    Ok(encode::clip(&lines, bounds))
}
fn rotate(q: V3, camera: Camera, angle: f64) -> V3 {
    let q = q - camera.world_center;
    let axis = camera.axes().2;
    let (s, c) = angle.to_radians().sin_cos();
    camera.world_center + q * c + axis.cross(q) * s + axis * (axis.dot(q) * (1. - c))
}
pub(super) fn wire(
    d: &Drawing,
    s: &Settings,
    paper: &crate::Paper,
    camera: Camera,
    hidden: bool,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Polyline>, String> {
    let mut render_camera = camera;
    render_camera.scale /= 1000.;
    render_camera.world_center = render_camera.world_center * 1000.;
    let mut scene = crate::scene::Scene::new(render_camera, area(paper));
    scene.set_intersections(false);
    scene.set_hidden_lines(hidden);
    let points = d.mesh.points();
    let transform = |q| rotate(q, camera, s.paper_rotation) * 1000.;
    for t in &d.mesh.triangles {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        scene.occluder(&t.map(|i| transform(points[i])));
    }
    for edge in feature_edges(&d.mesh, &camera, 25.) {
        scene.line(&edge.map(transform));
    }
    Ok(scene
        .render_cancellable(cancel)
        .ok_or("Cálculo cancelado.")?
        .parts
        .remove(0)
        .lines)
}
pub(super) fn views(
    d: &Drawing,
    s: &Settings,
    map: &ReadingMap,
    effect: &ReadingEffect,
    paper: &crate::Paper,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Polyline>>, String> {
    let mut out: Vec<Vec<Polyline>> = vec![vec![]; map.pens.len()];
    match *effect {
        ReadingEffect::Anaglyph {
            angle_deg,
            side_by_side,
        } => {
            for i in 0..2 {
                let mut camera = d.camera;
                camera.yaw_deg += (i as f64 - 0.5) * angle_deg;
                if side_by_side {
                    camera.scale *= 0.48;
                    camera.paper_center.x = paper.width_mm * (0.25 + 0.5 * i as f64);
                }
                let lines = wire(d, s, paper, camera, false, cancel)?;
                out[i % map.pens.len()].extend(lines);
            }
        }
        ReadingEffect::Spin { angle_deg, copies } => {
            for i in 0..copies {
                let mut camera = d.camera;
                camera.yaw_deg += i as f64 * angle_deg;
                let slot = i * map.pens.len() / copies;
                out[slot].extend(wire(d, s, paper, camera, false, cancel)?);
            }
        }
        _ => unreachable!(),
    }
    Ok(out)
}
pub(super) fn ridges(
    d: &Drawing,
    s: &Settings,
    map: &ReadingMap,
    geometry: &fields::Geometry,
    threshold: f64,
    paper: &crate::Paper,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Polyline>>, String> {
    let mut neighbors = vec![Vec::new(); d.mesh.vertices.len()];
    for t in &d.mesh.triangles {
        for j in 0..3 {
            let (a, b) = (t[j], t[(j + 1) % 3]);
            neighbors[a].push(b);
            neighbors[b].push(a);
        }
    }
    for n in &mut neighbors {
        n.sort_unstable();
        n.dedup();
    }
    // Curvatura suavizada y umbral sobre el percentil 98: el ruido de una malla esculpida
    // ni dibuja garabatos ni deja vacío el resto del modelo con un único pico.
    let curvature = fields::smoothed(&d.mesh, &geometry.mean);
    let magnitudes: Vec<f64> = curvature.iter().map(|v| v.abs()).collect();
    let max = fields::robust_range(&magnitudes)[1].max(1e-9);
    let mut score = vec![0.; neighbors.len()];
    for (i, n) in neighbors.iter().enumerate() {
        if n.is_empty() {
            continue;
        }
        let mean = n.iter().map(|j| magnitudes[*j]).sum::<f64>() / n.len() as f64;
        score[i] = magnitudes[i] - mean;
    }
    let camera = d.camera;
    let mut render_camera = camera;
    render_camera.scale /= 1000.;
    render_camera.world_center = render_camera.world_center * 1000.;
    let mut scene = crate::scene::Scene::new(render_camera, area(paper));
    scene.set_intersections(false);
    let parts = [scene.part("Crestas"), scene.part("Valles")];
    let points = d.mesh.points();
    let project = |q| rotate(q, camera, s.paper_rotation) * 1000.;
    for t in &d.mesh.triangles {
        scene.occluder(&t.map(|i| project(points[i])));
    }
    for (a, n) in neighbors.iter().enumerate() {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        for &b in n {
            if b <= a {
                continue;
            }
            let value = (curvature[a] + curvature[b]) * 0.5;
            if value.abs() < threshold * max || score[a].max(score[b]) <= 0. {
                continue;
            }
            scene.set_part(parts[usize::from(value < 0.)]);
            scene.line(&[project(points[a]), project(points[b])]);
        }
    }
    let mut out: Vec<Vec<Polyline>> = vec![vec![]; map.pens.len()];
    for (i, part) in scene
        .render_cancellable(cancel)
        .ok_or("Cálculo cancelado.")?
        .parts
        .into_iter()
        .skip(1)
        .enumerate()
    {
        out[i % map.pens.len()].extend(part.lines);
    }
    Ok(out)
}
/// Horizonte flotante de perfiles lineales, sin una rejilla de píxeles.
struct Horizon {
    area: BBox,
    cell: f64,
    bins: Vec<Vec<usize>>,
    segments: Vec<[P; 2]>,
}
impl Horizon {
    fn new(area: BBox) -> Self {
        Self {
            area,
            cell: 4.,
            bins: vec![vec![]; (area.width() / 4.).ceil() as usize + 1],
            segments: vec![],
        }
    }
    fn bins(&self, a: P, b: P) -> std::ops::RangeInclusive<usize> {
        let to = |x: f64| {
            (((x - self.area.min.x) / self.cell).floor().max(0.) as usize).min(self.bins.len() - 1)
        };
        to(a.x.min(b.x))..=to(a.x.max(b.x))
    }
    fn visible(&self, [a, b]: [P; 2]) -> Vec<Polyline> {
        let dx = b.x - a.x;
        if dx.abs() < 1e-10 {
            return vec![];
        }
        let mut ids = self
            .bins(a, b)
            .flat_map(|i| self.bins[i].iter().copied())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        let mut cuts = vec![0., 1.];
        let y = |[a, b]: [P; 2], x: f64| a.y + (b.y - a.y) * (x - a.x) / (b.x - a.x);
        for &id in &ids {
            let [c, d] = self.segments[id];
            for x in [c.x, d.x] {
                let t = (x - a.x) / dx;
                if t > 0. && t < 1. {
                    cuts.push(t);
                }
            }
            let slope = (b.y - a.y) / dx - (d.y - c.y) / (d.x - c.x);
            if slope.abs() > 1e-12 {
                let x =
                    (c.y - a.y + (b.y - a.y) * a.x / dx - (d.y - c.y) * c.x / (d.x - c.x)) / slope;
                let t = (x - a.x) / dx;
                if t > 0. && t < 1. && x >= c.x.min(d.x) && x <= c.x.max(d.x) {
                    cuts.push(t);
                }
            }
        }
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
        let mut out = Vec::new();
        for pair in cuts.windows(2) {
            let point = a.lerp(b, (pair[0] + pair[1]) * 0.5);
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for &id in &ids {
                let segment = self.segments[id];
                if point.x >= segment[0].x.min(segment[1].x) - 1e-9
                    && point.x <= segment[0].x.max(segment[1].x) + 1e-9
                {
                    let value = y(segment, point.x);
                    lo = lo.min(value);
                    hi = hi.max(value);
                }
            }
            if point.y < lo - 1e-8 || point.y > hi + 1e-8 {
                out.push(vec![a.lerp(b, pair[0]), a.lerp(b, pair[1])]);
            }
        }
        out
    }
    fn add(&mut self, segment: [P; 2]) {
        if (segment[1].x - segment[0].x).abs() < 1e-10 {
            return;
        }
        let id = self.segments.len();
        for i in self.bins(segment[0], segment[1]) {
            self.bins[i].push(id);
        }
        self.segments.push(segment);
    }
}
pub(super) fn horizons(
    d: &Drawing,
    map: &ReadingMap,
    height: f64,
    rows: usize,
    values: &fields::Values,
    geometry: &fields::Geometry,
    surface: &Surface,
    paper: &crate::Paper,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Polyline>>, String> {
    let bounds = area(paper);
    let mut profiles = Vec::new();
    for row in 0..rows {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let y = bounds.min.y + (row as f64 + 0.5) * bounds.height() / rows as f64;
        let mut segments = Vec::new();
        let mut depth = 0.;
        let mut count = 0;
        for segment in surface.segments(
            p(bounds.min.x, y),
            p(bounds.max.x, y),
            values,
            geometry,
            &d.mesh,
        ) {
            let z1 = d.camera.project(segment.a.world).1;
            let z2 = d.camera.project(segment.b.world).1;
            depth += z1 + z2;
            count += 2;
            let mut coded = vec![vec![]; map.pens.len()];
            encode::segment(segment, values, row, &mut coded);
            let dx = segment.b.point.x - segment.a.point.x;
            for (slot, lines) in coded.into_iter().enumerate() {
                for line in lines {
                    for pair in line.windows(2) {
                        let warp = |point: P| {
                            let t = if dx.abs() > 1e-12 {
                                (point.x - segment.a.point.x) / dx
                            } else {
                                0.
                            };
                            p(point.x, point.y - (z1 + (z2 - z1) * t) * height)
                        };
                        segments.push((slot, [warp(pair[0]), warp(pair[1])]));
                    }
                }
            }
        }
        if count > 0 {
            profiles.push((depth / count as f64, segments));
        }
    }
    profiles.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut horizon = Horizon::new(bounds);
    let mut out: Vec<Vec<Polyline>> = vec![vec![]; map.pens.len()];
    for (_, segments) in profiles {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        for (slot, segment) in &segments {
            for line in horizon.visible(*segment) {
                if let Some(previous) = out[*slot]
                    .last_mut()
                    .filter(|l| l.last().is_some_and(|p: &P| p.dist(line[0]) < 1e-7))
                {
                    previous.extend(line.into_iter().skip(1));
                } else {
                    out[*slot].push(line);
                }
            }
        }
        for (_, segment) in segments {
            horizon.add(segment);
        }
    }
    for lines in &mut out {
        *lines = encode::clip(lines, bounds);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn floating_horizon_keeps_only_the_exact_segments_outside_both_envelopes() {
        let mut horizon = Horizon::new(BBox {
            min: p(0., 0.),
            max: p(10., 10.),
        });
        horizon.add([p(0., 3.), p(10., 3.)]);
        horizon.add([p(0., 7.), p(10., 7.)]);
        assert_eq!(
            horizon.visible([p(0., 0.), p(10., 10.)]),
            vec![vec![p(0., 0.), p(3., 3.)], vec![p(7., 7.), p(10., 10.)]]
        );
        assert!(horizon.visible([p(0., 5.), p(10., 5.)]).is_empty());
    }
}
