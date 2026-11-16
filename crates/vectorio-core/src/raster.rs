//! Utilidades raster: rasterizado de anillos, distancia euclídea exacta, isolíneas
//! (marching squares), adelgazamiento Zhang–Suen y recorrido de esqueletos.

use crate::geom::{p, BBox, Polyline, P};
use rayon::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Grid<T> {
    pub w: usize,
    pub h: usize,
    pub data: Vec<T>,
}

impl<T: Copy + Default + Send + Sync> Grid<T> {
    pub fn new(w: usize, h: usize) -> Self {
        Grid { w, h, data: vec![T::default(); w * h] }
    }
    #[inline]
    pub fn get(&self, x: usize, y: usize) -> T {
        self.data[y * self.w + x]
    }
    #[inline]
    pub fn set(&mut self, x: usize, y: usize, v: T) {
        self.data[y * self.w + x] = v;
    }
    /// Lectura con borde: fuera de la rejilla devuelve el valor por defecto.
    #[inline]
    pub fn at(&self, x: isize, y: isize) -> T {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            T::default()
        } else {
            self.data[y as usize * self.w + x as usize]
        }
    }
}

/// Mapeo entre mm y píxeles de una rejilla que cubre `bbox` con `px` mm por píxel y un
/// margen de dos píxeles.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub origin: P,
    pub px: f64,
    pub w: usize,
    pub h: usize,
}

impl Frame {
    pub fn around(b: &BBox, px: f64) -> Frame {
        let origin = b.min - p(2.0 * px, 2.0 * px);
        let w = (b.width() / px).ceil() as usize + 5;
        let h = (b.height() / px).ceil() as usize + 5;
        Frame { origin, px, w, h }
    }
    pub fn to_mm(&self, q: P) -> P {
        self.origin + q * self.px
    }
}

/// Rasteriza anillos (par/impar) en una máscara; muestrea el centro de cada píxel.
pub fn rasterize(rings: &[Polyline], f: &Frame) -> Grid<bool> {
    let rings: Vec<_> = rings.iter().filter(|ring| ring.len() >= 3
        && ring.iter().all(|q| q.x.is_finite() && q.y.is_finite())).collect();
    let mut g = Grid::<bool>::new(f.w, f.h);
    let rows: Vec<Vec<bool>> = (0..f.h)
        .into_par_iter()
        .map(|y| {
            let yy = f.origin.y + (y as f64 + 0.5) * f.px;
            let mut xs: Vec<f64> = Vec::new();
            for ring in &rings {
                let n = ring.len();
                let ring_start = xs.len();
                let mut k = n - 1;
                for j in 0..n {
                    let (a, z) = (ring[k], ring[j]);
                    if (a.y > yy) != (z.y > yy) {
                        let x = a.x + (yy - a.y) * (z.x - a.x) / (z.y - a.y);
                        if !x.is_finite() {
                            xs.truncate(ring_start);
                            break;
                        }
                        xs.push(x);
                    }
                    k = j;
                }
            }
            xs.sort_by(f64::total_cmp);
            let mut row = vec![false; f.w];
            for pair in xs.chunks(2) {
                if pair.len() < 2 {
                    break;
                }
                let x0 = ((pair[0] - f.origin.x) / f.px - 0.5).ceil().max(0.0) as usize;
                let x1 = ((pair[1] - f.origin.x) / f.px - 0.5).floor();
                if x1 < 0.0 {
                    continue;
                }
                for pixel in row.iter_mut().take((x1 as usize).saturating_add(1)).skip(x0) {
                    *pixel = true;
                }
            }
            row
        })
        .collect();
    for (y, row) in rows.into_iter().enumerate() {
        g.data[y * f.w..(y + 1) * f.w].copy_from_slice(&row);
    }
    g
}

/// Transformada de distancia euclídea exacta (Felzenszwalb–Huttenlocher): distancia en
/// píxeles de cada píxel interior al fondo más cercano. El fondo vale 0.
pub fn distance_inside(mask: &Grid<bool>) -> Grid<f32> {
    let (w, h) = (mask.w, mask.h);
    const INF: f64 = 1e20;
    // Columnas.
    let mut tmp = vec![0f64; w * h];
    let cols: Vec<Vec<f64>> = (0..w)
        .into_par_iter()
        .map(|x| {
            let f: Vec<f64> = (0..h).map(|y| if mask.get(x, y) { INF } else { 0.0 }).collect();
            edt_1d(&f)
        })
        .collect();
    for (x, col) in cols.iter().enumerate() {
        for y in 0..h {
            tmp[y * w + x] = col[y];
        }
    }
    // Filas.
    let rows: Vec<Vec<f64>> = (0..h).into_par_iter().map(|y| edt_1d(&tmp[y * w..(y + 1) * w])).collect();
    let mut out = Grid::<f32>::new(w, h);
    for (y, row) in rows.into_iter().enumerate() {
        for (x, value) in row.into_iter().enumerate() {
            out.data[y * w + x] = value.sqrt() as f32;
        }
    }
    out
}

fn edt_1d(f: &[f64]) -> Vec<f64> {
    let n = f.len();
    let mut d = vec![0.0; n];
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f64; n + 1];
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    for q in 1..n {
        loop {
            let vk = v[k];
            let s = ((f[q] + (q * q) as f64) - (f[vk] + (vk * vk) as f64)) / (2.0 * q as f64 - 2.0 * vk as f64);
            if s <= z[k] {
                if k == 0 {
                    // No debería ocurrir con z[0] = −∞, pero evita desbordes.
                    v[0] = q;
                    z[1] = f64::INFINITY;
                    break;
                }
                k -= 1;
                continue;
            }
            k += 1;
            v[k] = q;
            z[k] = s;
            z[k + 1] = f64::INFINITY;
            break;
        }
    }
    k = 0;
    for (q, value) in d.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let dq = q as f64 - v[k] as f64;
        *value = dq * dq + f[v[k]];
    }
    d
}

/// Isolíneas del campo al nivel dado, en coordenadas de píxel (centros en x + 0,5).
/// Devuelve polilíneas cerradas (primer punto repetido) o abiertas si tocan el borde.
pub fn contours(field: &Grid<f32>, level: f32) -> Vec<Polyline> {
    let (w, h) = (field.w, field.h);
    if w < 2 || h < 2 {
        return vec![];
    }
    // Clave de arista: (x, y, horizontal?) del segmento entre centros de píxel.
    type Key = (u32, u32, bool);
    let interp = |k: Key| -> P {
        let (x, y, horiz) = (k.0 as usize, k.1 as usize, k.2);
        let (x2, y2) = if horiz { (x + 1, y) } else { (x, y + 1) };
        let a = field.get(x, y) - level;
        let b = field.get(x2, y2) - level;
        let t = if (a - b).abs() < 1e-12 { 0.5 } else { (a / (a - b)) as f64 };
        let t = t.clamp(0.0, 1.0);
        p(x as f64 + 0.5 + (x2 as f64 - x as f64) * t, y as f64 + 0.5 + (y2 as f64 - y as f64) * t)
    };
    // Segmentos por celda, en paralelo por filas.
    let segs: Vec<(Key, Key)> = (0..h - 1)
        .into_par_iter()
        .flat_map_iter(|y| {
            let mut local = Vec::new();
            for x in 0..w - 1 {
                let v = [
                    field.get(x, y) > level,
                    field.get(x + 1, y) > level,
                    field.get(x + 1, y + 1) > level,
                    field.get(x, y + 1) > level,
                ];
                let idx = (v[0] as u8) | (v[1] as u8) << 1 | (v[2] as u8) << 2 | (v[3] as u8) << 3;
                let (xu, yu) = (x as u32, y as u32);
                let top = (xu, yu, true);
                let right = (xu + 1, yu, false);
                let bottom = (xu, yu + 1, true);
                let left = (xu, yu, false);
                let pairs: &[(Key, Key)] = match idx {
                    0 | 15 => &[],
                    1 | 14 => &[(left, top)],
                    2 | 13 => &[(top, right)],
                    3 | 12 => &[(left, right)],
                    4 | 11 => &[(right, bottom)],
                    6 | 9 => &[(top, bottom)],
                    7 | 8 => &[(left, bottom)],
                    5 => &[(left, top), (right, bottom)],
                    10 => &[(top, right), (left, bottom)],
                    _ => &[],
                };
                local.extend_from_slice(pairs);
            }
            local
        })
        .collect();
    // Unión de segmentos en polilíneas.
    let mut adj: HashMap<Key, Vec<usize>> = HashMap::with_capacity(segs.len() * 2);
    for (i, (a, b)) in segs.iter().enumerate() {
        adj.entry(*a).or_default().push(i);
        adj.entry(*b).or_default().push(i);
    }
    let mut used = vec![false; segs.len()];
    let mut out = Vec::new();
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut chain: Vec<Key> = vec![segs[start].0, segs[start].1];
        // Extender hacia delante y luego hacia atrás.
        for dir in 0..2 {
            loop {
                let end = *chain.last().unwrap();
                let next = adj.get(&end).and_then(|v| v.iter().copied().find(|&i| !used[i]));
                match next {
                    Some(i) => {
                        used[i] = true;
                        let (a, b) = segs[i];
                        chain.push(if a == end { b } else { a });
                    }
                    None => break,
                }
            }
            if dir == 0 {
                chain.reverse();
            }
        }
        let pts: Polyline = chain.into_iter().map(interp).collect();
        if pts.len() >= 2 {
            out.push(pts);
        }
    }
    out
}

/// Adelgazamiento de Zhang–Suen: deja un esqueleto de un píxel de ancho.
pub fn thin(mask: &mut Grid<bool>) {
    let (w, h) = (mask.w as isize, mask.h as isize);
    loop {
        let mut changed = false;
        for step in 0..2 {
            let marks: Vec<usize> = (1..h - 1)
                .into_par_iter()
                .flat_map_iter(|y| {
                    let m = &*mask;
                    let mut local = Vec::new();
                    for x in 1..w - 1 {
                        if !m.at(x, y) {
                            continue;
                        }
                        let n = [
                            m.at(x, y - 1),
                            m.at(x + 1, y - 1),
                            m.at(x + 1, y),
                            m.at(x + 1, y + 1),
                            m.at(x, y + 1),
                            m.at(x - 1, y + 1),
                            m.at(x - 1, y),
                            m.at(x - 1, y - 1),
                        ];
                        let b = n.iter().filter(|v| **v).count();
                        if !(2..=6).contains(&b) {
                            continue;
                        }
                        let a = (0..8).filter(|&i| !n[i] && n[(i + 1) % 8]).count();
                        if a != 1 {
                            continue;
                        }
                        let ok = if step == 0 {
                            !(n[0] && n[2] && n[4]) && !(n[2] && n[4] && n[6])
                        } else {
                            !(n[0] && n[2] && n[6]) && !(n[0] && n[4] && n[6])
                        };
                        if ok {
                            local.push(y as usize * mask.w + x as usize);
                        }
                    }
                    local
                })
                .collect();
            if !marks.is_empty() {
                changed = true;
                for i in marks {
                    mask.data[i] = false;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

const N8: [(isize, isize); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];

/// Recorre un esqueleto de un píxel y lo convierte en polilíneas (en píxeles).
pub fn trace_skeleton(mask: &Grid<bool>) -> Vec<Polyline> {
    let (w, h) = (mask.w, mask.h);
    let degree = |x: usize, y: usize| -> usize {
        N8.iter().filter(|(dx, dy)| mask.at(x as isize + dx, y as isize + dy)).count()
    };
    let mut visited = Grid::<bool>::new(w, h);
    // Aristas visitadas como pares ordenados de índices para no repetir tramos.
    let mut out = Vec::new();
    let walk = |sx: usize, sy: usize, visited: &mut Grid<bool>, out: &mut Vec<Polyline>| {
        for &(dx, dy) in &N8 {
            let (nx, ny) = (sx as isize + dx, sy as isize + dy);
            if !mask.at(nx, ny) || visited.at(nx, ny) {
                continue;
            }
            let mut line = vec![p(sx as f64 + 0.5, sy as f64 + 0.5)];
            visited.set(sx, sy, true);
            let (mut cx, mut cy) = (nx as usize, ny as usize);
            loop {
                visited.set(cx, cy, true);
                line.push(p(cx as f64 + 0.5, cy as f64 + 0.5));
                if degree(cx, cy) > 2 {
                    break;
                }
                // Preferir vecinos ortogonales para no saltar esquinas.
                let mut next = None;
                for &(ex, ey) in N8.iter().step_by(2).chain(N8.iter().skip(1).step_by(2)) {
                    let (qx, qy) = (cx as isize + ex, cy as isize + ey);
                    if mask.at(qx, qy) && !visited.at(qx, qy) {
                        next = Some((qx as usize, qy as usize));
                        break;
                    }
                }
                match next {
                    Some((qx, qy)) => {
                        cx = qx;
                        cy = qy;
                    }
                    None => break,
                }
            }
            if line.len() >= 2 {
                out.push(line);
            }
        }
    };
    // Primero desde extremos y uniones, después ciclos aislados.
    for pass in 0..2 {
        for y in 0..h {
            for x in 0..w {
                if !mask.get(x, y) {
                    continue;
                }
                let d = degree(x, y);
                let start = if pass == 0 { d != 2 } else { !visited.get(x, y) };
                if start {
                    walk(x, y, &mut visited, &mut out);
                }
            }
        }
    }
    out
}

/// Umbral de Otsu sobre un histograma de grises.
pub fn otsu(gray: &[u8]) -> u8 {
    let mut hist = [0u64; 256];
    for &g in gray {
        hist[g as usize] += 1;
    }
    let total = gray.len() as f64;
    let sum: f64 = hist.iter().enumerate().map(|(i, &c)| i as f64 * c as f64).sum();
    let (mut sum_b, mut w_b, mut best, mut thr) = (0.0, 0.0, 0.0, 127u8);
    for (t, &c) in hist.iter().enumerate() {
        w_b += c as f64;
        if w_b == 0.0 {
            continue;
        }
        let w_f = total - w_b;
        if w_f == 0.0 {
            break;
        }
        sum_b += t as f64 * c as f64;
        let m_b = sum_b / w_b;
        let m_f = (sum - sum_b) / w_f;
        let between = w_b * w_f * (m_b - m_f).powi(2);
        if between > best {
            best = between;
            thr = t as u8;
        }
    }
    thr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_rings_do_not_pollute_the_mask_or_panic() {
        let frame = Frame { origin: p(0.0, 0.0), px: 1.0, w: 10, h: 10 };
        let good = vec![vec![p(2.0, 2.0), p(8.0, 2.0), p(8.0, 8.0), p(2.0, 8.0)]];
        let expected = rasterize(&good, &frame);
        for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut rings = good.clone();
            rings.push(vec![]);
            rings.push(vec![p(x, 0.0), p(5.0, 10.0), p(0.0, 0.0)]);
            assert_eq!(rasterize(&rings, &frame).data, expected.data);
        }
        let extreme = vec![vec![p(f64::MAX, -f64::MAX), p(-f64::MAX, f64::MAX), p(0.0, 0.0)]];
        assert!(rasterize(&extreme, &frame).data.iter().all(|v| !v));
        assert_eq!(rasterize(&[good[0].clone(), extreme[0].clone()], &frame).data, expected.data);
        assert!(rasterize(&good, &Frame { w: 0, ..frame }).data.is_empty());
    }

    #[test]
    fn edt_of_square_peaks_in_center() {
        let mut m = Grid::<bool>::new(21, 21);
        for y in 1..20 {
            for x in 1..20 {
                m.set(x, y, true);
            }
        }
        let d = distance_inside(&m);
        assert!((d.get(10, 10) - 10.0).abs() < 1e-3);
        assert_eq!(d.get(0, 0), 0.0);
    }

    #[test]
    fn contour_of_disc_is_closed() {
        let mut f = Grid::<f32>::new(30, 30);
        for y in 0..30 {
            for x in 0..30 {
                let d = ((x as f32 - 15.0).powi(2) + (y as f32 - 15.0).powi(2)).sqrt();
                f.set(x, y, 10.0 - d);
            }
        }
        let c = contours(&f, 0.0);
        assert_eq!(c.len(), 1);
        assert!(c[0][0].dist(*c[0].last().unwrap()) < 1e-9);
    }
}
