//! Fronteras de rampa interpoladas sobre segmentos; difuminado ordenado por trazo.
use super::surface::{Segment, Surface};
use super::*;
pub(super) fn slot(config: &ReadingMap, t: f64, index: usize) -> Option<usize> {
    let n = config.pens.len();
    if config.ramp == Ramp::Diverging {
        if (t - 0.5).abs() < 0.035 {
            return None;
        }
        let left = (n / 2).max(1);
        let choose = |x: f64, count: usize| -> usize {
            if count < 2 {
                return 0;
            }
            if config.encoding == Encoding::Dither {
                let x = x.clamp(0., 1.) * (count - 1) as f64;
                let lo = x.floor() as usize;
                (lo + usize::from(x - lo as f64 > rank(index))).min(count - 1)
            } else {
                ((x * count as f64).floor() as usize).min(count - 1)
            }
        };
        return Some(if t < 0.5 {
            choose(t / 0.465, left)
        } else {
            left + choose((t - 0.535) / 0.465, n - left)
        });
    }
    match config.encoding {
        Encoding::Bands | Encoding::Isolines | Encoding::Optical => {
            Some(((t * n as f64).floor() as usize).min(n - 1))
        }
        Encoding::Dither => {
            let x = t * (n - 1) as f64;
            let lo = x.floor() as usize;
            let fraction = x - lo as f64;
            Some((lo + usize::from(fraction > rank(index))).min(n - 1))
        }
        Encoding::Density => (rank(index) < t).then_some(0),
    }
}
pub(super) fn rank(index: usize) -> f64 {
    const ORDER: [usize; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
    (ORDER[index % 16] as f64 + 0.5) / 16.
}
fn cuts(config: &ReadingMap, values: &fields::Values, index: usize) -> Vec<f64> {
    if let Some(t) = config.threshold {
        return vec![t];
    }
    let n = config.pens.len();
    let normalized = if config.ramp == Ramp::Diverging {
        let left = (n / 2).max(1);
        let right = n - left;
        let local = |count: usize| -> Vec<f64> {
            if config.encoding == Encoding::Dither {
                if count < 2 {
                    vec![]
                } else {
                    (0..count - 1)
                        .map(|i| (i as f64 + rank(index)) / (count - 1) as f64)
                        .collect()
                }
            } else {
                (1..count).map(|i| i as f64 / count as f64).collect()
            }
        };
        local(left)
            .into_iter()
            .map(|t| 0.465 * t)
            .chain([0.465, 0.535])
            .chain(local(right).into_iter().map(|t| 0.535 + 0.465 * t))
            .collect::<Vec<_>>()
    } else {
        match config.encoding {
            Encoding::Bands | Encoding::Isolines | Encoding::Optical => {
                (1..n).map(|i| i as f64 / n as f64).collect()
            }
            Encoding::Dither => {
                if n < 2 {
                    vec![]
                } else {
                    (0..n - 1)
                        .map(|i| (i as f64 + rank(index)) / (n - 1) as f64)
                        .collect()
                }
            }
            Encoding::Density => vec![rank(index)],
        }
    };
    normalized.into_iter().map(|t| values.raw_at(t)).collect()
}
pub(super) fn segment(
    segment: Segment,
    values: &fields::Values,
    index: usize,
    out: &mut [Vec<Polyline>],
) {
    let config = &values.config;
    if config.measure == Measure::Objects {
        let slot = (segment.a.value.round() as usize) % out.len();
        push(&mut out[slot], segment.a.point, segment.b.point);
        return;
    }
    let mut split = vec![0., 1.];
    let delta = segment.b.value - segment.a.value;
    if delta.abs() > 1e-14 {
        let mut boundaries = cuts(config, values, index);
        if config.measure == Measure::Aspect && config.secondary.is_none() {
            let original = boundaries.clone();
            boundaries.extend(original.iter().map(|v| v - 360.));
            boundaries.extend(original.iter().map(|v| v + 360.));
            boundaries.extend([-360., 0., 360., 720.]);
        }
        for cut in boundaries {
            let t = (cut - segment.a.value) / delta;
            if t > 1e-10 && t < 1. - 1e-10 {
                split.push(t);
            }
        }
    }
    split.sort_by(f64::total_cmp);
    split.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
    for part in split.windows(2) {
        let a = part[0];
        let b = part[1];
        let value = segment.a.value + delta * (a + b) * 0.5;
        if let Some(slot) = slot(config, values.normalize(value), index) {
            push(
                &mut out[slot],
                segment.a.point.lerp(segment.b.point, a),
                segment.a.point.lerp(segment.b.point, b),
            );
        }
    }
}
pub(super) fn lines(
    surface: &Surface,
    d: &Drawing,
    input: &[Polyline],
    values: &fields::Values,
    geometry: &fields::Geometry,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Polyline>>, String> {
    let mut out = vec![vec![]; values.config.pens.len()];
    for (index, line) in input.iter().enumerate() {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        for pair in line.windows(2) {
            for part in surface.segments(pair[0], pair[1], values, geometry, &d.mesh) {
                segment(part, values, index, &mut out);
            }
        }
        if out.iter().map(Vec::len).sum::<usize>() > 250_000 {
            return Err("El mapa supera 250.000 trazos; reduce la densidad.".into());
        }
    }
    Ok(out)
}
pub(super) fn push(out: &mut Vec<Polyline>, a: P, b: P) {
    if a.dist(b) < 0.025 {
        return;
    }
    if let Some(last) = out
        .last_mut()
        .filter(|l| l.last().is_some_and(|p| p.dist(a) < 1e-8))
    {
        last.push(b);
    } else {
        out.push(vec![a, b]);
    }
}
pub(super) fn clip(lines: &[Polyline], area: BBox) -> Vec<Polyline> {
    let polygon = [
        area.min,
        p(area.max.x, area.min.y),
        area.max,
        p(area.min.x, area.max.y),
    ];
    let mut out = Vec::new();
    for line in lines {
        let mut current = Vec::new();
        for pair in line.windows(2) {
            if let Some((lo, hi)) = surface::interval(pair[0], pair[1], &polygon) {
                let a = pair[0].lerp(pair[1], lo);
                let b = pair[0].lerp(pair[1], hi);
                if current.last().is_some_and(|q: &P| q.dist(a) > 1e-8) {
                    if current.len() > 1 {
                        out.push(std::mem::take(&mut current));
                    } else {
                        current.clear();
                    }
                }
                if current.is_empty() {
                    current.push(a);
                }
                current.push(b);
            } else if current.len() > 1 {
                out.push(std::mem::take(&mut current));
            }
        }
        if current.len() > 1 {
            out.push(current);
        }
    }
    out
}
