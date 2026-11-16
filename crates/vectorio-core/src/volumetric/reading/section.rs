//! Corte por un semiespacio, tapa rayada con huecos y ocultación por la tapa analítica.
use super::*;
fn project(q: V3, d: &Drawing, s: &Settings) -> (P, f64) {
    let (p, z) = d.camera.project(q);
    (
        d.camera.paper_center + (p - d.camera.paper_center).rotate(s.paper_rotation.to_radians()),
        z,
    )
}
fn ring_intervals(a: P, b: P, rings: &[Polyline]) -> Vec<(f64, f64)> {
    let cross = |a: P, b: P| a.x * b.y - a.y * b.x;
    let mut cuts = vec![0., 1.];
    let d = b - a;
    for ring in rings {
        for pair in ring.windows(2) {
            let e = pair[1] - pair[0];
            let den = cross(d, e);
            if den.abs() < 1e-12 {
                continue;
            }
            let t = cross(pair[0] - a, e) / den;
            let u = cross(pair[0] - a, d) / den;
            if (0.0..=1.).contains(&t) && (0.0..=1.).contains(&u) {
                cuts.push(t);
            }
        }
    }
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
    cuts.windows(2)
        .filter_map(|p| {
            crate::geom::inside_rings(a.lerp(b, (p[0] + p[1]) * 0.5), rings).then_some((p[0], p[1]))
        })
        .collect()
}
pub(super) fn draw(
    d: &Drawing,
    s: &Settings,
    map: &ReadingMap,
    direction: [f64; 3],
    offset: f64,
    show_dimensions: bool,
    paper: &crate::Paper,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Polyline>>, String> {
    let normal = v3(direction).normalized();
    let points = d.mesh.points();
    let values = points.iter().map(|p| p.dot(normal)).collect::<Vec<_>>();
    let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let level = (lo + hi) * 0.5 + offset * (hi - lo) * 0.5;
    let contours = isolines(&d.mesh, &values, &[level], cancel)?;
    let rings = contours
        .iter()
        .filter(|c| c.points.first() == c.points.last())
        .map(|c| c.points.clone())
        .collect::<Vec<_>>();
    let projected_rings = rings
        .iter()
        .map(|ring| ring.iter().map(|q| project(*q, d, s).0).collect())
        .collect::<Vec<Polyline>>();
    let camera = d.camera;
    let mut render_camera = camera;
    render_camera.scale /= 1000.;
    render_camera.world_center = render_camera.world_center * 1000.;
    let axis = camera.axes().2;
    let rotate = |q: V3| {
        let rel = q - camera.world_center;
        let (sin, cos) = s.paper_rotation.to_radians().sin_cos();
        (camera.world_center
            + rel * cos
            + axis.cross(rel) * sin
            + axis * (axis.dot(rel) * (1. - cos)))
            * 1000.
    };
    let mut visible = crate::scene::Scene::new(render_camera, effects::area(paper));
    visible.set_intersections(false);
    let mut hidden = crate::scene::Scene::new(render_camera, effects::area(paper));
    hidden.set_intersections(false);
    hidden.set_hidden_lines(true);
    let mut occluders = Vec::new();
    let mut cut_bounds = [
        V3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY),
        V3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
    ];
    for t in &d.mesh.triangles {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let tri = t.map(|i| points[i]);
        let mut clipped = Vec::new();
        for i in 0..3 {
            let a = tri[i];
            let b = tri[(i + 1) % 3];
            let va = a.dot(normal) - level;
            let vb = b.dot(normal) - level;
            if va <= 0. {
                clipped.push(a);
            }
            if (va <= 0.) != (vb <= 0.) {
                clipped.push(a + (b - a) * (va / (va - vb)));
            }
        }
        for q in &clipped {
            cut_bounds[0] = V3::new(
                cut_bounds[0].x.min(q.x),
                cut_bounds[0].y.min(q.y),
                cut_bounds[0].z.min(q.z),
            );
            cut_bounds[1] = V3::new(
                cut_bounds[1].x.max(q.x),
                cut_bounds[1].y.max(q.y),
                cut_bounds[1].z.max(q.z),
            );
        }
        for i in 1..clipped.len().saturating_sub(1) {
            let face = [
                rotate(clipped[0]),
                rotate(clipped[i]),
                rotate(clipped[i + 1]),
            ];
            visible.occluder(&face);
            hidden.occluder(&face);
            occluders.push(face);
        }
    }
    let mut cap_hidden = Vec::new();
    let (right, down, away) = camera.axes();
    let den = normal.dot(away);
    for [mut a, mut b] in feature_edges(&d.mesh, &camera, 25.) {
        let va = a.dot(normal) - level;
        let vb = b.dot(normal) - level;
        if va > 0. && vb > 0. {
            continue;
        }
        if (va > 0.) != (vb > 0.) {
            let q = a + (b - a) * (va / (va - vb));
            if va > 0. {
                a = q;
            } else {
                b = q;
            }
        }
        let (pa, za) = project(a, d, s);
        let (pb, zb) = project(b, d, s);
        let mut cuts = vec![0., 1.];
        let intervals = ring_intervals(pa, pb, &projected_rings);
        for (lo, hi) in &intervals {
            cuts.extend([*lo, *hi]);
        }
        let cap_depth = |point: P| {
            let q = (point - camera.paper_center).rotate(-s.paper_rotation.to_radians())
                * (1. / camera.scale);
            (level - normal.dot(camera.world_center + right * q.x + down * q.y)) / den
        };
        if den.abs() > 1e-9 {
            let da = za - cap_depth(pa);
            let db = zb - cap_depth(pb);
            if (da > 0.) != (db > 0.) {
                cuts.push((da / (da - db)).clamp(0., 1.));
            }
        }
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
        for part in cuts.windows(2) {
            let mid = (part[0] + part[1]) * 0.5;
            let covered = den.abs() > 1e-9
                && intervals.iter().any(|(lo, hi)| mid >= *lo && mid <= *hi)
                && (za + (zb - za) * mid) > cap_depth(pa.lerp(pb, mid)) + 1e-7;
            let aa = a + (b - a) * part[0];
            let bb = a + (b - a) * part[1];
            if covered {
                cap_hidden.push(vec![project(aa, d, s).0, project(bb, d, s).0]);
            } else {
                visible.line(&[rotate(aa), rotate(bb)]);
                hidden.line(&[rotate(aa), rotate(bb)]);
            }
        }
    }
    for contour in &contours {
        visible.line(
            &contour
                .points
                .iter()
                .map(|q| rotate(*q))
                .collect::<Vec<_>>(),
        );
    }
    let mut out = vec![vec![]; map.pens.len()];
    out[0] = visible
        .render_cancellable(cancel)
        .ok_or("Cálculo cancelado.")?
        .parts
        .remove(0)
        .lines;
    let mut hidden_lines = hidden
        .render_cancellable(cancel)
        .ok_or("Cálculo cancelado.")?
        .parts
        .remove(0)
        .lines;
    hidden_lines.extend(cap_hidden);
    out[1 % map.pens.len()] =
        super::super::shadow::tone::dashed(&hidden_lines, DashPattern::default(), cancel)?;
    let u = if normal.z.abs() < 0.9 {
        normal.cross(V3::new(0., 0., 1.)).normalized()
    } else {
        normal.cross(V3::new(1., 0., 0.)).normalized()
    };
    let v = normal.cross(u);
    let origin = normal * level;
    let plane_rings = rings
        .iter()
        .map(|ring| ring.iter().map(|q| p(q.dot(u), q.dot(v))).collect())
        .collect::<Vec<Polyline>>();
    let bounds = BBox::of(&plane_rings);
    if !bounds.is_empty() {
        let step = map.spacing_mm / camera.scale;
        let mut hatch = crate::scene::Scene::new(render_camera, effects::area(paper));
        hatch.set_intersections(false);
        // La oclusión de la tapa se obtiene reutilizando el resultado visible de su perímetro y las caras recortadas.
        for face in &occluders {
            hatch.occluder(face);
        }

        for j in (bounds.min.y / step).ceil() as i64..=(bounds.max.y / step).floor() as i64 {
            if cancel() {
                return Err("Cálculo cancelado.".into());
            }
            let y = j as f64 * step;
            let a = p(bounds.min.x, y);
            let b = p(bounds.max.x, y);
            for (lo, hi) in ring_intervals(a, b, &plane_rings) {
                let a = a.lerp(b, lo);
                let b = p(bounds.min.x, y).lerp(p(bounds.max.x, y), hi);
                hatch.line(&[
                    rotate(origin + u * a.x + v * a.y),
                    rotate(origin + u * b.x + v * b.y),
                ]);
            }
        }
        out[2 % map.pens.len()].extend(
            hatch
                .render_cancellable(cancel)
                .ok_or("Cálculo cancelado.")?
                .parts
                .remove(0)
                .lines,
        );
    }
    if show_dimensions && cut_bounds.iter().all(|p| p.finite()) {
        let size = cut_bounds[1] - cut_bounds[0];
        legend::text(
            &mut out[0],
            &format!("X {:.3} u  Y {:.3} u  Z {:.3} u", size.x, size.y, size.z),
            effects::area(paper).min + p(2., 2.),
            2.,
            effects::area(paper).width() - 4.,
        );
    }
    for lines in &mut out {
        *lines = encode::clip(lines, effects::area(paper));
    }
    Ok(out)
}
