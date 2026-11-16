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
    let chains = chains(lines);
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
            let next = shifted(line, spacing * copy as f64 * sign);
            points += next.len();
            if points > 2_000_000 || out.len() >= 250_000 {
                return Err("Demasiados ecos; reduce las repeticiones.".into());
            }
            out.push(next);
        }
    }
    Ok(out)
}
