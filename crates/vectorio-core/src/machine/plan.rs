//! Planificador de movimiento: convierte una polilínea en mm en tramos de velocidad
//! constante para la EiBotBoard (orden `SM`: duración entera en ms y pasos por motor).
//!
//! 1. Velocidad máxima en cada vértice por desviación de esquina (como grbl): una
//!    curva suave se recorre sin frenar y una esquina viva casi para.
//! 2. Pasadas hacia atrás y hacia delante para que acelerar y frenar quepan en cada
//!    segmento; cada segmento queda con un perfil trapezoidal.
//! 3. Muestreo del recorrido cada `SLICE_S`. Un vértice que se saldría de la cuerda más
//!    de `CHORD_TOL_MM` corta el tramo justo en él, así las esquinas salen exactas.
//! 4. Los pasos se calculan en posición absoluta, así el redondeo nunca se acumula.

use crate::{p, P};

/// La placa admite 25 000 pasos/s por motor; se deja margen.
pub const MAX_STEP_RATE: f64 = 24_000.0;
/// Duración nominal de cada tramo. La placa encola dos órdenes, así que con 20 ms el
/// ordenador va como mucho 40 ms por delante y una parada es inmediata.
pub const SLICE_S: f64 = 0.020;
/// Distancia máxima entre la cuerda de un tramo y los vértices que salta.
pub const CHORD_TOL_MM: f64 = 0.02;

/// Mecánica en H de AxiDraw (y MakerQ): motor1 = X + Y, motor2 = X − Y.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kinematics {
    pub steps_per_mm: f64,
}

impl Kinematics {
    pub fn motors(&self, q: P) -> (i64, i64) {
        let (x, y) = (q.x * self.steps_per_mm, q.y * self.steps_per_mm);
        ((x + y).round() as i64, (x - y).round() as i64)
    }

    pub fn point(&self, m: (i64, i64)) -> P {
        let k = 2.0 * self.steps_per_mm;
        p((m.0 + m.1) as f64 / k, (m.0 - m.1) as f64 / k)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// Velocidad de crucero, mm/s.
    pub speed: f64,
    /// Aceleración, mm/s².
    pub accel: f64,
    /// Desviación de esquina, mm: cuanto mayor, más rápido toma las esquinas.
    pub junction_mm: f64,
}

impl Limits {
    pub fn valid(&self) -> bool {
        [self.speed, self.accel].iter().all(|v| v.is_finite() && *v > 0.0)
            && self.junction_mm.is_finite() && self.junction_mm >= 0.0
    }
}

/// Un tramo `SM`: duración y pasos relativos de cada motor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub ms: u32,
    pub m1: i64,
    pub m2: i64,
}

struct Seg {
    a: P,
    b: P,
    len: f64,
    t0: f64,
    v0: f64,
    vc: f64,
    ta: f64,
    tc: f64,
    td: f64,
    da: f64,
    dc: f64,
}

impl Seg {
    fn duration(&self) -> f64 { self.ta + self.tc + self.td }

    fn dist(&self, tau: f64, acc: f64) -> f64 {
        let d = if tau <= 0.0 {
            0.0
        } else if tau < self.ta {
            self.v0 * tau + 0.5 * acc * tau * tau
        } else if tau < self.ta + self.tc {
            self.da + self.vc * (tau - self.ta)
        } else {
            let u = (tau - self.ta - self.tc).min(self.td);
            self.da + self.dc + self.vc * u - 0.5 * acc * u * u
        };
        d.clamp(0.0, self.len)
    }
}

/// Tramos para recorrer `path` (mm de máquina) desde la posición `from` (pasos),
/// parando al principio y al final.
pub fn plan(path: &[P], lim: &Limits, kin: &Kinematics, from: (i64, i64)) -> Vec<Move> {
    let pts = dedupe(path);
    if pts.len() < 2 || !lim.valid() { return vec![]; }
    let segs = profile(&pts, lim);
    let bounds = boundaries(&pts, &segs, lim.accel);
    to_moves(&bounds, kin, from)
}

/// Duración del recorrido sin cuantizar, s (para estimaciones).
pub fn duration(path: &[P], lim: &Limits) -> f64 {
    let pts = dedupe(path);
    if pts.len() < 2 || !lim.valid() { return 0.0; }
    profile(&pts, lim).last().map_or(0.0, |s| s.t0 + s.duration())
}

fn dedupe(path: &[P]) -> Vec<P> {
    let mut pts: Vec<P> = Vec::with_capacity(path.len());
    for &q in path {
        if pts.last().is_none_or(|l: &P| l.dist(q) > 1e-4) { pts.push(q); }
    }
    pts
}

fn profile(pts: &[P], lim: &Limits) -> Vec<Seg> {
    let n = pts.len() - 1;
    let (a, vmax) = (lim.accel, lim.speed);
    let lens: Vec<f64> = pts.windows(2).map(|w| w[0].dist(w[1])).collect();
    let dirs: Vec<P> = pts.windows(2).zip(&lens).map(|(w, &l)| (w[1] - w[0]) * (1.0 / l)).collect();
    let mut v = vec![0.0; n + 1];
    for k in 1..n {
        // Con el giro φ entre segmentos, sin(θ/2) = cos(φ/2) para el ángulo θ de la esquina.
        let c = dirs[k - 1].dot(dirs[k]).clamp(-1.0, 1.0);
        let s = ((1.0 + c) * 0.5).sqrt();
        v[k] = if s > 0.999_999 { vmax } else if s < 1e-6 { 0.0 } else { (a * lim.junction_mm * s / (1.0 - s)).sqrt().min(vmax) };
    }
    for k in (0..n).rev() { v[k] = v[k].min((v[k + 1] * v[k + 1] + 2.0 * a * lens[k]).sqrt()); }
    for k in 0..n { v[k + 1] = v[k + 1].min((v[k] * v[k] + 2.0 * a * lens[k]).sqrt()); }
    let mut segs = Vec::with_capacity(n);
    let mut t0 = 0.0;
    for k in 0..n {
        let (len, v0, v1) = (lens[k], v[k], v[k + 1]);
        let vc = ((2.0 * a * len + v0 * v0 + v1 * v1) / 2.0).sqrt().min(vmax).max(v0).max(v1);
        let da = ((vc * vc - v0 * v0) / (2.0 * a)).max(0.0);
        let dd = ((vc * vc - v1 * v1) / (2.0 * a)).max(0.0);
        let dc = (len - da - dd).max(0.0);
        let seg = Seg { a: pts[k], b: pts[k + 1], len, t0, v0, vc,
            ta: (vc - v0) / a, tc: if vc > 0.0 { dc / vc } else { 0.0 }, td: (vc - v1) / a, da, dc };
        t0 += seg.duration();
        segs.push(seg);
    }
    segs
}

fn point_at(segs: &[Seg], t: f64, acc: f64) -> P {
    let i = segs.partition_point(|g| g.t0 <= t).saturating_sub(1);
    let g = &segs[i];
    g.a.lerp(g.b, g.dist(t - g.t0, acc) / g.len)
}

fn seg_dist(q: P, a: P, b: P) -> f64 {
    let ab = b - a;
    let l2 = ab.dot(ab);
    let t = if l2 > 0.0 { ((q - a).dot(ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
    q.dist(a + ab * t)
}

fn deviation(a: P, b: P, pts: &[P]) -> f64 { pts.iter().map(|&q| seg_dist(q, a, b)).fold(0.0, f64::max) }

/// Instantes y puntos donde termina cada tramo.
fn boundaries(pts: &[P], segs: &[Seg], acc: f64) -> Vec<(f64, P)> {
    let last = segs.last().unwrap();
    let total = last.t0 + last.duration();
    let n = segs.len();
    let mut out = Vec::new();
    let (mut t, mut q) = (0.0, pts[0]);
    // Primer vértice interior todavía no alcanzado.
    let mut next = 1;
    while t < total - 1e-9 {
        let mut t1 = t + SLICE_S;
        if t1 > total - SLICE_S * 0.25 { t1 = total; }
        let mut q1 = if t1 >= total { pts[n] } else { point_at(segs, t1, acc) };
        while next < n && segs[next].t0 <= t + 1e-12 { next += 1; }
        let mut end = next;
        while end < n && segs[end].t0 < t1 - 1e-12 { end += 1; }
        // Vértices interiores next..end.
        if end > next && deviation(q, q1, &pts[next..end]) > CHORD_TOL_MM {
            let mut best = next;
            for j in next + 1..end {
                if deviation(q, pts[j], &pts[next..j]) <= CHORD_TOL_MM { best = j } else { break }
            }
            t1 = segs[best].t0;
            q1 = pts[best];
        }
        out.push((t1, q1));
        t = t1;
        q = q1;
    }
    out
}

fn to_moves(bounds: &[(f64, P)], kin: &Kinematics, from: (i64, i64)) -> Vec<Move> {
    let mut out = Vec::with_capacity(bounds.len());
    let (mut prev_ms, mut prev) = (0_i64, from);
    for (i, &(t, q)) in bounds.iter().enumerate() {
        let last = i + 1 == bounds.len();
        let ms = (t * 1000.0).round() as i64;
        let m = kin.motors(q);
        let (d1, d2) = (m.0 - prev.0, m.1 - prev.1);
        // Un tramo de menos de 1 ms o sin pasos se suma al siguiente.
        if (ms <= prev_ms || (d1 == 0 && d2 == 0)) && !last { continue; }
        if d1 == 0 && d2 == 0 { break; }
        let need = (d1.abs().max(d2.abs()) as f64 * 1000.0 / MAX_STEP_RATE).ceil() as i64;
        let dur = (ms - prev_ms).max(1).max(need);
        out.push(Move { ms: dur as u32, m1: d1, m2: d2 });
        prev_ms = ms;
        prev = m;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const KIN: Kinematics = Kinematics { steps_per_mm: 80.0 };
    const LIM: Limits = Limits { speed: 40.0, accel: 1000.0, junction_mm: 0.03 };

    fn replay(moves: &[Move], from: (i64, i64)) -> Vec<(i64, i64)> {
        let mut at = from;
        moves.iter().map(|m| { at = (at.0 + m.m1, at.1 + m.m2); at }).collect()
    }

    fn check_rates(moves: &[Move]) {
        for m in moves {
            assert!(m.ms >= 1);
            assert!(m.m1.abs().max(m.m2.abs()) as f64 * 1000.0 / m.ms as f64 <= MAX_STEP_RATE + 1e-9, "{m:?}");
        }
    }

    #[test]
    fn kinematics_round_trip() {
        let q = p(12.3, -4.5);
        let m = KIN.motors(q);
        assert_eq!(m, (repeat(7.8), repeat(16.8)));
        let back = KIN.point(m);
        assert!(back.dist(q) < 1.0 / 80.0);
        fn repeat(mm: f64) -> i64 { (mm * 80.0).round() as i64 }
    }

    #[test]
    fn straight_line_reaches_target_with_ramp() {
        let moves = plan(&[p(0.0, 0.0), p(50.0, 0.0)], &LIM, &KIN, (0, 0));
        check_rates(&moves);
        assert_eq!(*replay(&moves, (0, 0)).last().unwrap(), KIN.motors(p(50.0, 0.0)));
        // Arranca y para con suavidad: el primer y el último tramo son cortos.
        let speed = |m: &Move| (m.m1.abs() as f64 / 80.0) / (m.ms as f64 / 1000.0);
        assert!(speed(&moves[0]) < 15.0, "{:?}", moves[0]);
        assert!(speed(moves.last().unwrap()) < 15.0);
        let cruise = speed(&moves[moves.len() / 2]);
        assert!((cruise - 40.0).abs() < 2.0, "crucero {cruise}");
        // 50 mm a 40 mm/s con rampas de 0,04 s: ~1,29 s.
        let total: u32 = moves.iter().map(|m| m.ms).sum();
        assert!((1250..1350).contains(&total), "{total} ms");
    }

    #[test]
    fn acceleration_is_bounded() {
        let moves = plan(&[p(0.0, 0.0), p(30.0, 20.0)], &Limits { speed: 150.0, ..LIM }, &KIN, (0, 0));
        let v: Vec<f64> = moves.iter().map(|m| {
            let q = KIN.point((m.m1, m.m2));
            q.len() / (m.ms as f64 / 1000.0)
        }).collect();
        for w in v.windows(2) {
            // Entre centros de tramos de 20 ms; un paso de cuantización suma ~0,6 mm/s.
            assert!((w[1] - w[0]).abs() <= 1000.0 * SLICE_S * 1.1 + 2.0, "{w:?}");
        }
    }

    #[test]
    fn square_corners_are_exact() {
        let sq = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0), p(0.0, 0.0)];
        let moves = plan(&sq, &LIM, &KIN, (0, 0));
        check_rates(&moves);
        let at = replay(&moves, (0, 0));
        for corner in &sq[1..] {
            assert!(at.contains(&KIN.motors(*corner)), "falta la esquina {corner:?}");
        }
        assert_eq!(*at.last().unwrap(), (0, 0));
    }

    #[test]
    fn dense_curve_stays_on_path() {
        let circle: Vec<P> = (0..=720).map(|i| {
            let a = i as f64 / 720.0 * std::f64::consts::TAU;
            p(20.0 + 15.0 * a.cos(), 20.0 + 15.0 * a.sin())
        }).collect();
        let from = KIN.motors(circle[0]);
        let moves = plan(&circle, &LIM, &KIN, from);
        check_rates(&moves);
        for at in replay(&moves, from) {
            let r = KIN.point(at).dist(p(20.0, 20.0));
            assert!((r - 15.0).abs() < CHORD_TOL_MM + 0.02, "radio {r}");
        }
        // Una curva suave no obliga a cortar en cada vértice.
        assert!(moves.len() < 200, "{} tramos", moves.len());
    }

    #[test]
    fn respects_motor_rate_limit() {
        let moves = plan(&[p(0.0, 0.0), p(200.0, 200.0)], &Limits { speed: 1000.0, accel: 20_000.0, junction_mm: 0.03 }, &KIN, (0, 0));
        check_rates(&moves);
        assert_eq!(*replay(&moves, (0, 0)).last().unwrap(), KIN.motors(p(200.0, 200.0)));
    }

    #[test]
    fn degenerate_paths_produce_nothing() {
        assert!(plan(&[p(1.0, 1.0)], &LIM, &KIN, (0, 0)).is_empty());
        assert!(plan(&[p(1.0, 1.0), p(1.0, 1.0)], &LIM, &KIN, (0, 0)).is_empty());
        assert!(plan(&[p(0.0, 0.0), p(1.0, 0.0)], &Limits { speed: 0.0, ..LIM }, &KIN, (0, 0)).is_empty());
    }
}
