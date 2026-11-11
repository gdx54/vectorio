//! Geometría básica en milímetros: puntos, cajas, transformaciones y utilidades de polilíneas.

use serde::{Deserialize, Serialize};
use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct P {
    pub x: f64,
    pub y: f64,
}

pub const fn p(x: f64, y: f64) -> P {
    P { x, y }
}

impl P {
    pub fn len(self) -> f64 {
        self.x.hypot(self.y)
    }
    pub fn dist(self, o: P) -> f64 {
        (self - o).len()
    }
    pub fn dot(self, o: P) -> f64 {
        self.x * o.x + self.y * o.y
    }
    pub fn lerp(self, o: P, t: f64) -> P {
        self + (o - self) * t
    }
    pub fn rotate(self, rad: f64) -> P {
        let (s, c) = rad.sin_cos();
        p(self.x * c - self.y * s, self.x * s + self.y * c)
    }
}

impl Add for P {
    type Output = P;
    fn add(self, o: P) -> P {
        p(self.x + o.x, self.y + o.y)
    }
}
impl Sub for P {
    type Output = P;
    fn sub(self, o: P) -> P {
        p(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f64> for P {
    type Output = P;
    fn mul(self, k: f64) -> P {
        p(self.x * k, self.y * k)
    }
}

pub type Polyline = Vec<P>;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BBox {
    pub min: P,
    pub max: P,
}

impl BBox {
    pub fn empty() -> Self {
        BBox { min: p(f64::INFINITY, f64::INFINITY), max: p(f64::NEG_INFINITY, f64::NEG_INFINITY) }
    }
    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x
    }
    pub fn add(&mut self, q: P) {
        self.min.x = self.min.x.min(q.x);
        self.min.y = self.min.y.min(q.y);
        self.max.x = self.max.x.max(q.x);
        self.max.y = self.max.y.max(q.y);
    }
    pub fn union(&mut self, o: &BBox) {
        if !o.is_empty() {
            self.add(o.min);
            self.add(o.max);
        }
    }
    pub fn of<'a>(lines: impl IntoIterator<Item = &'a Polyline>) -> Self {
        let mut b = BBox::empty();
        for l in lines {
            for q in l {
                b.add(*q);
            }
        }
        b
    }
    pub fn width(&self) -> f64 {
        (self.max.x - self.min.x).max(0.0)
    }
    pub fn height(&self) -> f64 {
        (self.max.y - self.min.y).max(0.0)
    }
    pub fn center(&self) -> P {
        self.min.lerp(self.max, 0.5)
    }
    pub fn contains(&self, q: P, tol: f64) -> bool {
        q.x >= self.min.x - tol && q.x <= self.max.x + tol && q.y >= self.min.y - tol && q.y <= self.max.y + tol
    }
}

/// Posición, escala y giro de un objeto: local → mundo.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub pos: P,
    pub scale: f64,
    pub rotation_deg: f64,
}

impl Default for Placement {
    fn default() -> Self {
        Placement { pos: P::default(), scale: 1.0, rotation_deg: 0.0 }
    }
}

impl Placement {
    pub fn apply(&self, q: P) -> P {
        (q * self.scale).rotate(self.rotation_deg.to_radians()) + self.pos
    }
    pub fn inverse(&self, q: P) -> P {
        (q - self.pos).rotate(-self.rotation_deg.to_radians()) * (1.0 / self.scale.max(1e-9))
    }
    pub fn apply_all(&self, lines: &[Polyline]) -> Vec<Polyline> {
        lines.iter().map(|l| l.iter().map(|q| self.apply(*q)).collect()).collect()
    }
}

pub fn length(l: &[P]) -> f64 {
    l.windows(2).map(|w| w[0].dist(w[1])).sum()
}

pub fn signed_area(ring: &[P]) -> f64 {
    let n = ring.len();
    if n < 3 {
        return 0.0;
    }
    let mut a = 0.0;
    for i in 0..n {
        let (u, v) = (ring[i], ring[(i + 1) % n]);
        a += u.x * v.y - v.x * u.y;
    }
    a * 0.5
}

/// Par/impar sobre todos los anillos: los agujeros quedan fuera.
pub fn inside_rings(q: P, rings: &[Polyline]) -> bool {
    let mut odd = false;
    for ring in rings {
        let n = ring.len();
        if n < 3 {
            continue;
        }
        let mut k = n - 1;
        for j in 0..n {
            let (a, z) = (ring[k], ring[j]);
            if (a.y > q.y) != (z.y > q.y) && q.x < (z.x - a.x) * (q.y - a.y) / (z.y - a.y) + a.x {
                odd = !odd;
            }
            k = j;
        }
    }
    odd
}

fn seg_dist(q: P, a: P, b: P) -> f64 {
    let ab = b - a;
    let l2 = ab.dot(ab);
    if l2 < 1e-18 {
        return q.dist(a);
    }
    let t = ((q - a).dot(ab) / l2).clamp(0.0, 1.0);
    q.dist(a + ab * t)
}

/// Distancia mínima de un punto a un conjunto de polilíneas.
pub fn dist_to_lines(q: P, lines: &[Polyline]) -> f64 {
    let mut best = f64::INFINITY;
    for l in lines {
        if l.len() == 1 {
            best = best.min(q.dist(l[0]));
        }
        for w in l.windows(2) {
            best = best.min(seg_dist(q, w[0], w[1]));
        }
    }
    best
}

/// Ramer–Douglas–Peucker.
pub fn simplify(pts: &[P], eps: f64) -> Polyline {
    if pts.len() < 3 || eps <= 0.0 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0usize, pts.len() - 1)];
    while let Some((s, e)) = stack.pop() {
        let mut best = (0.0, 0usize);
        for i in s + 1..e {
            let d = seg_dist(pts[i], pts[s], pts[e]);
            if d > best.0 {
                best = (d, i);
            }
        }
        if best.0 > eps {
            keep[best.1] = true;
            stack.push((s, best.1));
            stack.push((best.1, e));
        }
    }
    pts.iter().zip(keep).filter(|(_, k)| *k).map(|(q, _)| *q).collect()
}

/// Suavizado de Chaikin. En curvas cerradas el primer punto se repite al final.
pub fn chaikin(pts: &[P], iterations: usize, closed: bool) -> Polyline {
    let mut cur = pts.to_vec();
    for _ in 0..iterations {
        if cur.len() < 3 {
            break;
        }
        let mut next = Vec::with_capacity(cur.len() * 2);
        let n = cur.len();
        if closed {
            let ring = if cur[0].dist(cur[n - 1]) < 1e-9 { &cur[..n - 1] } else { &cur[..] };
            let m = ring.len();
            for i in 0..m {
                let (a, b) = (ring[i], ring[(i + 1) % m]);
                next.push(a.lerp(b, 0.25));
                next.push(a.lerp(b, 0.75));
            }
            next.push(next[0]);
        } else {
            next.push(cur[0]);
            for i in 0..n - 1 {
                let (a, b) = (cur[i], cur[i + 1]);
                next.push(a.lerp(b, 0.25));
                next.push(a.lerp(b, 0.75));
            }
            next.push(cur[n - 1]);
        }
        cur = next;
    }
    cur
}

/// Catmull–Rom centrípeta muestreada; `closed` envuelve los extremos.
pub fn catmull_rom(pts: &[P], samples: usize, closed: bool) -> Polyline {
    let n = pts.len();
    if n < 3 || samples < 2 {
        let mut out = pts.to_vec();
        if closed && n > 1 {
            out.push(pts[0]);
        }
        return out;
    }
    let get = |i: isize| -> P {
        if closed {
            pts[i.rem_euclid(n as isize) as usize]
        } else if i < 0 {
            pts[0] + (pts[0] - pts[1])
        } else if i as usize >= n {
            pts[n - 1] + (pts[n - 1] - pts[n - 2])
        } else {
            pts[i as usize]
        }
    };
    let segs = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(segs * samples + 1);
    for s in 0..segs {
        let i = s as isize;
        let (p0, p1, p2, p3) = (get(i - 1), get(i), get(i + 1), get(i + 2));
        for k in 0..samples {
            let t = k as f64 / samples as f64;
            out.push(cr_point(p0, p1, p2, p3, t));
        }
    }
    out.push(if closed { pts[0] } else { pts[n - 1] });
    out
}

fn cr_point(p0: P, p1: P, p2: P, p3: P, t: f64) -> P {
    // Parametrización centrípeta (alfa = 0,5) para evitar bucles y picos.
    let tj = |a: P, b: P| a.dist(b).sqrt().max(1e-6);
    let t0 = 0.0;
    let t1 = t0 + tj(p0, p1);
    let t2 = t1 + tj(p1, p2);
    let t3 = t2 + tj(p2, p3);
    let tt = t1 + (t2 - t1) * t;
    let a1 = p0 * ((t1 - tt) / (t1 - t0)) + p1 * ((tt - t0) / (t1 - t0));
    let a2 = p1 * ((t2 - tt) / (t2 - t1)) + p2 * ((tt - t1) / (t2 - t1));
    let a3 = p2 * ((t3 - tt) / (t3 - t2)) + p3 * ((tt - t2) / (t3 - t2));
    let b1 = a1 * ((t2 - tt) / (t2 - t0)) + a2 * ((tt - t0) / (t2 - t0));
    let b2 = a2 * ((t3 - tt) / (t3 - t1)) + a3 * ((tt - t1) / (t3 - t1));
    b1 * ((t2 - tt) / (t2 - t1)) + b2 * ((tt - t1) / (t2 - t1))
}

/// Generador pseudoaleatorio pequeño y determinista (splitmix64).
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniforme en [0, 1).
    pub fn f(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Uniforme en [-1, 1).
    pub fn sym(&mut self) -> f64 {
        self.f() * 2.0 - 1.0
    }
    /// Aproximadamente normal (suma de tres uniformes), media 0 y desviación ~1.
    pub fn gauss(&mut self) -> f64 {
        (self.sym() + self.sym() + self.sym()) * 1.0
    }
}
