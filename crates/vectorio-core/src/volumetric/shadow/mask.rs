//! Unión de máscaras por intervalos, con fase global en mm. No rasteriza ni
//! repasa las juntas: objeto y suelo comparten una sola pasada por línea.
use crate::{geom::Polyline, p, BBox, P};
fn merge(intervals: &mut Vec<(f64, f64)>) {
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut n = 0;
    for i in 0..intervals.len() {
        let next = intervals[i];
        if n > 0 && next.0 <= intervals[n - 1].1 + 1e-8 {
            intervals[n - 1].1 = intervals[n - 1].1.max(next.1);
        } else {
            intervals[n] = next;
            n += 1;
        }
    }
    intervals.truncate(n);
}
fn difference(a: &[(f64, f64)], b: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let mut j = 0;
    for &(lo, hi) in a {
        let mut at = lo;
        while j < b.len() && b[j].1 <= at {
            j += 1;
        }
        let mut k = j;
        while k < b.len() && b[k].0 < hi {
            if b[k].0 > at {
                out.push((at, b[k].0.min(hi)));
            }
            at = at.max(b[k].1);
            if at >= hi {
                break;
            }
            k += 1;
        }
        if at < hi {
            out.push((at, hi));
        }
    }
    out
}
struct Ring {
    points: Vec<P>,
    min: f64,
    max: f64,
}
struct Rings {
    rings: Vec<Ring>,
}
impl Rings {
    fn new(polygons: &[Polyline], along: P, normal: P) -> Self {
        Self {
            rings: polygons
                .iter()
                .filter(|p| p.len() >= 3)
                .map(|poly| {
                    let points: Vec<_> = poly
                        .iter()
                        .map(|p| crate::p(p.dot(along), p.dot(normal)))
                        .collect();
                    let min = points.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
                    let max = points.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
                    Ring { points, min, max }
                })
                .collect(),
        }
    }
    fn intervals(&self, height: f64) -> Vec<(f64, f64)> {
        let mut intervals = Vec::new();
        for ring in &self.rings {
            if height < ring.min || height >= ring.max {
                continue;
            }
            let mut hits = Vec::new();
            for (a, b) in ring
                .points
                .iter()
                .zip(ring.points.iter().cycle().skip(1))
                .take(ring.points.len())
            {
                if (a.y > height) != (b.y > height) {
                    hits.push(a.x + (height - a.y) * (b.x - a.x) / (b.y - a.y));
                }
            }
            hits.sort_by(f64::total_cmp);
            for pair in hits.chunks_exact(2) {
                if pair[1] - pair[0] > 1e-9 {
                    intervals.push((pair[0], pair[1]));
                }
            }
        }
        merge(&mut intervals);
        intervals
    }
}
fn clip_segment(a: P, b: P, area: BBox) -> Option<(P, P)> {
    let d = b - a;
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for (start, delta) in [
        (a.x - area.min.x, d.x),
        (area.max.x - a.x, -d.x),
        (a.y - area.min.y, d.y),
        (area.max.y - a.y, -d.y),
    ] {
        if delta.abs() < 1e-14 {
            if start < 0. {
                return None;
            }
        } else {
            let t = -start / delta;
            if delta > 0. {
                lo = lo.max(t);
            } else {
                hi = hi.min(t);
            }
            if hi <= lo {
                return None;
            }
        }
    }
    Some((a.lerp(b, lo), a.lerp(b, hi)))
}
pub(super) fn hatch(
    dark: &[Polyline],
    middle: &[Polyline],
    ground: &[Polyline],
    object: &[Polyline],
    area: BBox,
    angle: f64,
    spacing: f64,
    cancel: &dyn Fn() -> bool,
) -> Result<[Vec<Polyline>; 2], String> {
    let (s, c) = angle.to_radians().sin_cos();
    let along = p(c, s);
    let normal = p(-s, c);
    let masks = [
        Rings::new(dark, along, normal),
        Rings::new(middle, along, normal),
        Rings::new(ground, along, normal),
        Rings::new(object, along, normal),
    ];
    let corners = [
        area.min,
        p(area.max.x, area.min.y),
        area.max,
        p(area.min.x, area.max.y),
    ];
    let lo = corners
        .iter()
        .map(|p| p.dot(normal))
        .fold(f64::INFINITY, f64::min);
    let hi = corners
        .iter()
        .map(|p| p.dot(normal))
        .fold(f64::NEG_INFINITY, f64::max);
    let mut output = [Vec::new(), Vec::new()];
    for i in (lo / spacing).ceil() as i64..=(hi / spacing).floor() as i64 {
        if cancel() {
            return Err(super::CANCEL.into());
        }
        let height = i as f64 * spacing;
        let own = masks[0].intervals(height);
        let floor = masks[2].intervals(height);
        let silhouette = masks[3].intervals(height);
        let mut shadow = own;
        shadow.extend(difference(&floor, &silhouette));
        merge(&mut shadow);
        let mid = difference(&masks[1].intervals(height), &shadow);
        for (role, intervals) in [shadow, mid].iter().enumerate() {
            for &(lo, hi) in intervals {
                let a = along * lo + normal * height;
                let b = along * hi + normal * height;
                if let Some((a, b)) = clip_segment(a, b, area) {
                    if a.dist(b) >= 0.025 {
                        output[role].push(vec![a, b]);
                    }
                }
            }
        }
        if output.iter().map(Vec::len).sum::<usize>() > 250_000 {
            return Err(
                "La trama supera 250.000 trazos; aumenta la separación del relleno.".into(),
            );
        }
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn union_joins_touching_shadow_and_never_overprints() {
        let left = vec![p(0., 0.), p(5., 0.), p(5., 10.), p(0., 10.)];
        let right = vec![p(5., 0.), p(10., 0.), p(10., 10.), p(5., 10.)];
        let area = BBox {
            min: p(0., 0.),
            max: p(10., 10.),
        };
        let out = hatch(
            &[left.clone()],
            &[],
            &[left, right.clone()],
            &[],
            area,
            0.,
            1.,
            &|| false,
        )
        .unwrap();
        assert_eq!(out[0].len(), 10);
        assert!(out[0].iter().all(|l| (l[0].dist(l[1]) - 10.).abs() < 1e-9));
        let clipped = hatch(&[], &[], &[right.clone()], &[right], area, 0., 1., &|| {
            false
        })
        .unwrap();
        assert!(clipped[0].is_empty());
    }
}
