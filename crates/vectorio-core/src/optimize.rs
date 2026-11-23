//! Orden de trazos y estadísticas de ploteo.

use crate::doc::{PlotLayer, PlotSettings};
use crate::geom::{length, Polyline, P};
use std::collections::HashMap;

const CELL: f64 = 8.0;

fn cell(q: P) -> (i32, i32) {
    ((q.x / CELL).floor() as i32, (q.y / CELL).floor() as i32)
}

/// Vecino más cercano con inversión de sentido, usando una rejilla de extremos.
pub fn order(paths: Vec<Polyline>, start: P) -> Vec<Polyline> {
    let n = paths.len();
    if n < 2 {
        return paths;
    }
    let mut grid: HashMap<(i32, i32), Vec<(usize, bool)>> = HashMap::new();
    for (i, l) in paths.iter().enumerate() {
        grid.entry(cell(l[0])).or_default().push((i, false));
        grid.entry(cell(l[l.len() - 1])).or_default().push((i, true));
    }
    let mut used = vec![false; n];
    let mut out = Vec::with_capacity(n);
    let mut cur = start;
    let mut paths: Vec<Option<Polyline>> = paths.into_iter().map(Some).collect();
    for _ in 0..n {
        let (cx, cy) = cell(cur);
        let mut best: Option<(f64, usize, bool)> = None;
        let mut r = 0i32;
        loop {
            // Anillo de celdas a distancia r.
            for dx in -r..=r {
                for dy in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    if let Some(v) = grid.get_mut(&(cx + dx, cy + dy)) {
                        v.retain(|(i, _)| !used[*i]);
                        for &(i, rev) in v.iter() {
                            let l = paths[i].as_ref().unwrap();
                            let q = if rev { l[l.len() - 1] } else { l[0] };
                            let d = q.dist(cur);
                            if best.is_none_or(|b| d < b.0) {
                                best = Some((d, i, rev));
                            }
                        }
                    }
                }
            }
            if let Some(b) = best {
                if b.0 <= r as f64 * CELL {
                    break;
                }
            }
            r += 1;
            if r > 4096 {
                break;
            }
        }
        let Some((_, i, rev)) = best else { break };
        used[i] = true;
        let mut l = paths[i].take().unwrap();
        if rev {
            l.reverse();
        }
        cur = l[l.len() - 1];
        out.push(l);
    }
    out
}

/// Une trazos consecutivos cuyo final coincide con el inicio del siguiente.
pub fn join(paths: Vec<Polyline>, tol: f64) -> Vec<Polyline> {
    let mut out: Vec<Polyline> = Vec::with_capacity(paths.len());
    for l in paths {
        if let Some(last) = out.last_mut() {
            if last[last.len() - 1].dist(l[0]) <= tol {
                last.extend(l.into_iter().skip(1));
                continue;
            }
        }
        out.push(l);
    }
    out
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub draw_mm: f64,
    pub travel_mm: f64,
    pub lifts: usize,
    pub pen_changes: usize,
    pub seconds: f64,
}

/// Estimación de tiempo y recorridos. Cada plumilla empieza y termina en el origen.
pub fn stats(layers: &[PlotLayer], s: &PlotSettings) -> Stats { stats_for(layers, s, &crate::output::Dynamics::default()) }

/// Como `stats`, con los tiempos de esta máquina: acelera, frena en las esquinas y sube y
/// baja la pluma en cada trazo que no empieza donde acabó el anterior.
pub fn stats_for(layers: &[PlotLayer], s: &PlotSettings, dynamics: &crate::output::Dynamics) -> Stats {
    let motion = PlotSettings { draw_speed: s.draw_speed.max(1.0), travel_speed: s.travel_speed.max(1.0), ..s.clone() };
    let timing = crate::output::Timing::new(&motion, dynamics);
    let mut st = Stats::default();
    for layer in layers {
        let (mut cur, mut pen_up) = (P::default(), true);
        for l in &layer.paths {
            if l.is_empty() { continue; }
            st.travel_mm += cur.dist(l[0]);
            st.draw_mm += length(l);
            st.lifts += 1;
            let hop = timing.hop(cur, l[0]);
            if hop > 0.0 && !pen_up { st.seconds += timing.lift; pen_up = true; }
            if pen_up { st.seconds += timing.lift; pen_up = false; }
            st.seconds += hop + timing.draw(l);
            cur = l[l.len() - 1];
        }
        st.travel_mm += cur.dist(P::default());
        st.seconds += timing.hop(cur, P::default()) + if pen_up { 0.0 } else { timing.lift };
    }
    st.pen_changes = layers.len().saturating_sub(1);
    st
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::p;

    #[test]
    fn order_reverses_to_shorten_travel() {
        let a = vec![p(0.0, 0.0), p(10.0, 0.0)];
        let b = vec![p(20.0, 0.0), p(11.0, 0.0)];
        let o = order(vec![b, a], p(0.0, 0.0));
        assert_eq!(o[0][0], p(0.0, 0.0));
        assert_eq!(o[1][0], p(11.0, 0.0));
        let j = join(o, 1.5);
        assert_eq!(j.len(), 1);
    }
}
