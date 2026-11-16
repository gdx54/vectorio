//! Ruido suave de posición y huecos deterministas. No depende del orden ni de la tinta.
use super::*;
use crate::P;
pub(super) fn mix(mut v: u64) -> u64 {
    v = v.wrapping_add(0x9e3779b97f4a7c15);
    v = (v ^ (v >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    v = (v ^ (v >> 27)).wrapping_mul(0x94d049bb133111eb);
    v ^ (v >> 31)
}
fn unit(v: u64) -> f64 {
    (mix(v) >> 11) as f64 / (1u64 << 53) as f64
}
fn noise(point: P, wave: f64, seed: u64) -> f64 {
    let x = point.x / wave;
    let y = point.y / wave;
    let ix = x.floor() as i64;
    let iy = y.floor() as i64;
    let smooth = |t: f64| t * t * (3. - 2. * t);
    let fx = smooth(x - x.floor());
    let fy = smooth(y - y.floor());
    let at = |x: i64, y: i64| {
        unit(seed ^ mix(x as u64) ^ mix((y as u64).wrapping_add(0x1234abcd))) * 2. - 1.
    };
    let a = at(ix, iy) * (1. - fx) + at(ix + 1, iy) * fx;
    let b = at(ix, iy + 1) * (1. - fx) + at(ix + 1, iy + 1) * fx;
    a * (1. - fy) + b * fy
}
fn moved(point: P, s: &FinishSettings, seed: u64) -> P {
    point
        + p(
            noise(point, s.wavelength_mm, seed),
            noise(point, s.wavelength_mm, seed ^ 0x4ee15a11),
        ) * (s.jitter_mm * std::f64::consts::FRAC_1_SQRT_2)
}
fn identity(line: &Polyline) -> u64 {
    let a = line.first().copied().unwrap_or_default();
    let b = line.last().copied().unwrap_or_default();
    let key =
        |p: P| mix((p.x * 1e6).round() as i64 as u64) ^ mix((p.y * 1e6).round() as i64 as u64);
    mix(key(a) ^ key(b) ^ mix((crate::geom::length(line) * 1e6).round() as u64))
}
fn subline(line: &Polyline, lo: f64, hi: f64) -> Polyline {
    let mut out = Vec::new();
    let mut distance = 0.;
    for pair in line.windows(2) {
        let len = pair[0].dist(pair[1]);
        if len < 1e-12 {
            continue;
        }
        let a = ((lo - distance) / len).clamp(0., 1.);
        let b = ((hi - distance) / len).clamp(0., 1.);
        if b > a + 1e-12 {
            if out.is_empty() {
                out.push(pair[0].lerp(pair[1], a));
            }
            out.push(pair[0].lerp(pair[1], b));
        }
        distance += len;
    }
    out
}
pub(super) fn lines(
    line: &Polyline,
    s: &FinishSettings,
    seed: u64,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Polyline>, String> {
    if !s.variation() {
        return Ok(vec![line.clone()]);
    }
    if line.len() < 2 {
        return Ok(vec![]);
    }
    let total = crate::geom::length(line);
    let closed = line[0].dist(*line.last().unwrap()) < 1e-8;
    let key = seed ^ identity(line);
    let trim = if closed {
        0.
    } else {
        total * 0.2 * s.irregularity
    };
    let start = trim * unit(key);
    let end = total - trim * unit(key ^ 0xcafebab3);
    let mut intervals = vec![(start, end)];
    if s.gap_probability > 0. {
        intervals.clear();
        let cell = s.gap_mm * 8.;
        let mut at = start;
        for i in 0..=(total / cell).ceil() as u64 {
            if cancel() {
                return Err("Cálculo cancelado.".into());
            }
            if unit(key ^ mix(i)) >= s.gap_probability {
                continue;
            }
            let center = (i as f64 + 0.25 + unit(key ^ mix(i) ^ 0x82a9) * 0.5) * cell;
            let length = s.gap_mm * (0.5 + unit(key ^ mix(i) ^ 0x18a7));
            let lo = (center - length * 0.5).clamp(start, end);
            let hi = (center + length * 0.5).clamp(start, end);
            if lo > at {
                intervals.push((at, lo));
            }
            at = at.max(hi);
        }
        if at < end {
            intervals.push((at, end));
        }
    }
    let mut out = Vec::new();
    let mut budget = 0;
    for (lo, hi) in intervals {
        if hi - lo < 0.05 {
            continue;
        }
        let line = subline(line, lo, hi);
        if s.jitter_mm == 0. {
            out.push(line);
            continue;
        }
        let mut result = Vec::new();
        for pair in line.windows(2) {
            let len = pair[0].dist(pair[1]);
            let count = (len / (s.wavelength_mm * 0.25)).ceil().max(1.) as usize;
            if budget + count > 2_000_000 {
                return Err(
                    "El temblor supera dos millones de puntos; aumenta su longitud de onda.".into(),
                );
            }
            for i in 0..count {
                if i % 256 == 0 && cancel() {
                    return Err("Cálculo cancelado.".into());
                }
                result.push(moved(
                    pair[0].lerp(pair[1], i as f64 / count as f64),
                    s,
                    seed,
                ));
            }
            budget += count;
        }
        if let Some(p) = line.last() {
            result.push(moved(*p, s, seed));
        }
        out.push(result);
    }
    Ok(out)
}
