//! Ecos de contorno con inglete limitado. Cierra cadenas, conserva huecos y admite perfiles abiertos.
use super::*;
use crate::P;
use std::collections::BTreeMap;
fn key(p: P) -> (i64, i64) {
    ((p.x * 1e5).round() as i64, (p.y * 1e5).round() as i64)
}
fn chains(lines: &[Polyline]) -> Vec<Polyline> {
    let mut ends = BTreeMap::<(i64, i64), Vec<(usize, bool)>>::new();
    for (i, line) in lines.iter().enumerate() {
        if line.len() < 2 {
            continue;
        }
        ends.entry(key(line[0])).or_default().push((i, false));
        ends.entry(key(*line.last().unwrap()))
            .or_default()
            .push((i, true));
    }
    let mut used = vec![false; lines.len()];
    let mut out = Vec::new();
    for i in 0..lines.len() {
        if used[i] || lines[i].len() < 2 {
            continue;
        }
        used[i] = true;
        let mut line = lines[i].clone();
        for reverse in [false, true] {
            if reverse {
                line.reverse();
            }
            loop {
                let end = *line.last().unwrap();
                if line[0].dist(end) < 1e-5 {
                    let first = line[0];
                    *line.last_mut().unwrap() = first;
                    break;
                }
                let Some(next) = ends.get(&key(end)) else {
                    break;
                };
                let free = next.iter().filter(|(i, _)| !used[*i]).collect::<Vec<_>>();
                if free.len() != 1 {
                    break;
                }
                let &&(j, flip) = free.first().unwrap();
                used[j] = true;
                let mut part = lines[j].clone();
                if flip {
                    part.reverse();
                }
                line.extend(part.into_iter().skip(1));
            }
        }
        line.reverse();
        out.push(line);
    }
    out
}
/// Une extremos abiertos separados menos de `snap` que siguen la misma dirección: las
/// siluetas facetadas dejan pequeños saltos que el eco no debe repetir. Dos tramos de
/// sombreado contiguos no se unen porque el salto entre ellos es transversal.
fn bridge(chains: Vec<Polyline>, snap: f64) -> Vec<Polyline> {
    let open: Vec<usize> = (0..chains.len())
        .filter(|i| chains[*i].len() >= 2 && chains[*i][0].dist(*chains[*i].last().unwrap()) > 1e-7)
        .collect();
    if open.len() < 2 && open.iter().all(|i| length(&chains[*i]) < 4. * snap) || snap <= 0. {
        return chains;
    }
    // Extremo `2 * k + lado`: punto y tangente hacia fuera.
    let end = |k: usize| -> (P, P) {
        let line = &chains[open[k / 2]];
        let (tip, inner) = if k % 2 == 0 {
            (line[0], line[1])
        } else {
            (line[line.len() - 1], line[line.len() - 2])
        };
        let d = tip - inner;
        (tip, d * (1. / d.len().max(1e-12)))
    };
    let cell = |q: P| ((q.x / snap).floor() as i64, (q.y / snap).floor() as i64);
    let mut grid = BTreeMap::<(i64, i64), Vec<usize>>::new();
    for k in 0..open.len() * 2 {
        grid.entry(cell(end(k).0)).or_default().push(k);
    }
    let mut pairs = Vec::new();
    for k in 0..open.len() * 2 {
        let (a, ta) = end(k);
        let (cx, cy) = cell(a);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &m in grid.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    if m <= k {
                        continue;
                    }
                    let (b, tb) = end(m);
                    let gap = b - a;
                    let distance = gap.len();
                    if distance > snap {
                        continue;
                    }
                    // Sigue recto (hueco) o es un escalón: los dos extremos van en la misma
                    // dirección, casi superpuestos, y se recorta lo que se solapa.
                    let forward = distance < 1e-9
                        || gap.dot(ta) > 0.7 * distance && (gap * -1.).dot(tb) > 0.7 * distance;
                    let lateral = (ta.x * gap.y - ta.y * gap.x).abs();
                    let overlap = (-gap.dot(ta)).max(0.);
                    let step = ta.dot(tb) < -0.9 && lateral < 0.25 * snap && overlap > 0.;
                    let same = k / 2 == m / 2;
                    let room = |c: usize| length(&chains[open[c]]) > 4. * snap.max(overlap);
                    if (forward || step) && room(k / 2) && room(m / 2) && (!same || room(k / 2)) {
                        pairs.push((distance, k, m, if step { overlap * 0.5 } else { 0. }));
                    }
                }
            }
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut link = vec![usize::MAX; open.len() * 2];
    let mut cut = vec![0.; open.len() * 2];
    for (_, k, m, trim) in pairs {
        if link[k] == usize::MAX && link[m] == usize::MAX {
            link[k] = m;
            link[m] = k;
            cut[k] = trim;
            cut[m] = trim;
        }
    }
    let mut out: Vec<Polyline> = (0..chains.len())
        .filter(|i| !open.contains(i))
        .map(|i| chains[i].clone())
        .collect();
    let mut seen = vec![false; open.len()];
    // Primero los caminos que empiezan en un extremo libre; después los ciclos.
    let starts = (0..open.len() * 2)
        .filter(|k| link[*k] == usize::MAX)
        .chain(0..open.len() * 2);
    for start in starts {
        if seen[start / 2] {
            continue;
        }
        let mut line: Polyline = Vec::new();
        let mut k = start;
        loop {
            let chain = k / 2;
            if seen[chain] {
                break;
            }
            seen[chain] = true;
            let source = &chains[open[chain]];
            let mut part =
                cut_ends(source, cut[2 * chain], cut[2 * chain + 1]).unwrap_or_else(|| source.clone());
            if k % 2 == 1 {
                part.reverse();
            }
            line.extend(part);
            let exit = k ^ 1;
            if link[exit] == usize::MAX {
                break;
            }
            k = link[exit];
        }
        if link[start] != usize::MAX && line.len() > 2 {
            let first = line[0];
            line.push(first);
        }
        out.push(line);
    }
    out
}
fn length(line: &[P]) -> f64 {
    line.windows(2).map(|w| w[0].dist(w[1])).sum()
}
/// Recorta `cut` mm de cada extremo: los ecos de un trazo abierto se anidan sin
/// sobresalir por donde la silueta se corta.
fn trimmed(line: &Polyline, cut: f64) -> Option<Polyline> {
    if length(line) - 2. * cut < cut.max(0.05) {
        return None;
    }
    cut_ends(line, cut, cut)
}
fn cut_ends(line: &Polyline, start: f64, end: f64) -> Option<Polyline> {
    let total = length(line);
    if start + end <= 0. {
        return Some(line.clone());
    }
    if total - start - end < 0.05 {
        return None;
    }
    let (from, to) = (start, total - end);
    let mut out = Vec::new();
    let mut walked = 0.;
    for w in line.windows(2) {
        let step = w[0].dist(w[1]);
        let (a, b) = (walked, walked + step);
        if b >= from && a <= to && step > 1e-12 {
            let lo = ((from - a) / step).clamp(0., 1.);
            let hi = ((to - a) / step).clamp(0., 1.);
            if out.is_empty() {
                out.push(w[0].lerp(w[1], lo));
            }
            out.push(w[0].lerp(w[1], hi));
        }
        walked = b;
    }
    (out.len() >= 2).then_some(out)
}
fn shifted(line: &Polyline, d: f64) -> Polyline {
    let closed = line[0].dist(*line.last().unwrap()) < 1e-7;
    let n = line.len() - usize::from(closed);
    if n < 2 {
        return Vec::new();
    }
    let normal = |a: P, b: P| {
        let delta = b - a;
        let len = delta.len();
        if len > 1e-10 {
            p(-delta.y, delta.x) * (1. / len)
        } else {
            p(0., 0.)
        }
    };
    let mut out = Vec::new();
    for i in 0..n {
        let current = line[i];
        let prev = if i > 0 {
            line[i - 1]
        } else if closed {
            line[n - 1]
        } else {
            current
        };
        let next = if i + 1 < n {
            line[i + 1]
        } else if closed {
            line[0]
        } else {
            current
        };
        let mut a = normal(prev, current);
        let mut b = normal(current, next);
        if a.len() < 1e-10 {
            a = b;
        }
        if b.len() < 1e-10 {
            b = a;
        }
        let average = a + b;
        let denom = average.dot(a);
        if denom.abs() < 1e-8 {
            out.push(current + a * d);
            continue;
        }
        let shift = average * (d / denom);
        if shift.len() > d.abs() * 4. {
            out.push(current + a * d);
            out.push(current + b * d);
        } else {
            out.push(current + shift);
        }
    }
    if closed && !out.is_empty() {
        out.push(out[0]);
    }
    out
}
pub(super) fn copies(
    lines: &[Polyline],
    count: usize,
    spacing: f64,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Polyline>, String> {
    // El puente alcanza el grosor del eco: los saltos menores no se notan en el original.
    let chains = bridge(chains(lines), spacing.abs() * count.max(1) as f64);
    let mut out = Vec::new();
    let mut points = 0;
    for (i, line) in chains.iter().enumerate() {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let closed = line.first() == line.last();
        let sign = if closed {
            let depth = chains
                .iter()
                .enumerate()
                .filter(|(j, other)| {
                    *j != i
                        && other.first() == other.last()
                        && crate::geom::inside_rings(line[0], std::slice::from_ref(*other))
                })
                .count();
            -crate::geom::signed_area(line).signum() * if depth % 2 == 0 { 1. } else { -1. }
        } else {
            1.
        };
        for copy in 1..=count {
            let mut next = shifted(line, spacing * copy as f64 * sign);
            if !closed {
                match trimmed(&next, spacing.abs() * copy as f64) {
                    Some(cut) => next = cut,
                    None => break,
                }
            }
            points += next.len();
            if points > 2_000_000 || out.len() >= 250_000 {
                return Err("Demasiados ecos; reduce las repeticiones.".into());
            }
            out.push(next);
        }
    }
    Ok(out)
}
