//! Rellenos para plotter: líneas que el rotulador recorre dentro de formas cerradas.
//! Todo en mm y con la regla par/impar, de modo que los agujeros quedan abiertos.

use crate::geom::{inside_rings, p, simplify, BBox, Polyline, Rng, P};
use crate::raster;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FillStyle {
    None,
    /// Paralelas en una dirección.
    Hatch,
    /// Paralelas en dos direcciones.
    Cross,
    /// Paralelas unidas en un solo trazo continuo (menos subidas de pluma).
    Zigzag,
    /// Zigzag con la separación del grosor del rotulador: cubre la forma.
    Solid,
    /// Contornos hacia dentro, como curvas de nivel.
    Concentric,
    /// Puntillismo en rejilla hexagonal con ligera variación.
    Dots,
    /// Paralelas onduladas.
    Wave,
    /// El motor de trazos de las fotos: flujo, espiral, garabato, vibración…
    Trace,
}

impl FillStyle {
    pub const ALL: [FillStyle; 9] = [
        FillStyle::None,
        FillStyle::Hatch,
        FillStyle::Cross,
        FillStyle::Zigzag,
        FillStyle::Solid,
        FillStyle::Concentric,
        FillStyle::Dots,
        FillStyle::Wave,
        FillStyle::Trace,
    ];
    pub fn label(self) -> &'static str {
        match self {
            FillStyle::None => "Sin relleno",
            FillStyle::Hatch => "Rayado",
            FillStyle::Cross => "Cruzado",
            FillStyle::Zigzag => "Zigzag continuo",
            FillStyle::Solid => "Sólido",
            FillStyle::Concentric => "Concéntrico",
            FillStyle::Dots => "Puntillismo",
            FillStyle::Wave => "Ondas",
            FillStyle::Trace => "Efectos de trazo",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fill {
    pub style: FillStyle,
    /// Índice de la plumilla del relleno.
    pub pen: usize,
    pub spacing_mm: f64,
    pub angle_deg: f64,
    /// Margen interior respecto al contorno.
    pub inset_mm: f64,
    /// Con `FillStyle::Trace`: estilo, largo, grosor y vibración. La separación y el
    /// ángulo son los del relleno.
    #[serde(default)]
    pub trace: crate::trace::TraceParams,
    /// Con `FillStyle::Trace`: cuánto color pone el relleno (0–1).
    #[serde(default = "full_tone")]
    pub tone: f64,
}

fn full_tone() -> f64 {
    1.0
}

impl Default for Fill {
    fn default() -> Self {
        Fill { style: FillStyle::None, pen: 0, spacing_mm: 1.5, angle_deg: 45.0, inset_mm: 0.0, trace: Default::default(), tone: 1.0 }
    }
}

impl Fill {
    /// Ajustes del motor de trazos para este relleno.
    pub fn trace_params(&self) -> crate::trace::TraceParams {
        crate::trace::TraceParams { spacing_mm: self.spacing_mm, angle_deg: self.angle_deg, relay: false, ..self.trace.clone() }
    }
}

/// Líneas del relleno para los anillos dados (mm, mundo).
pub fn generate(rings: &[Polyline], fill: &Fill, pen_width: f64, seed: u64) -> Vec<Polyline> {
    let rings: Vec<Polyline> = rings.iter().filter(|r| r.len() >= 3).cloned().collect();
    if rings.is_empty() || fill.style == FillStyle::None {
        return vec![];
    }
    let spacing = match fill.style {
        FillStyle::Solid => (pen_width * 0.85).max(0.1),
        _ => fill.spacing_mm.max(0.1),
    };
    let inset = fill.inset_mm.max(0.0);
    match fill.style {
        FillStyle::None => vec![],
        FillStyle::Hatch => hatch(&rings, fill.angle_deg, spacing, inset, false),
        FillStyle::Cross => {
            let mut a = hatch(&rings, fill.angle_deg, spacing, inset, false);
            a.extend(hatch(&rings, fill.angle_deg + 90.0, spacing, inset, false));
            a
        }
        FillStyle::Zigzag | FillStyle::Solid => hatch(&rings, fill.angle_deg, spacing, inset, true),
        FillStyle::Concentric => concentric(&rings, spacing, inset),
        FillStyle::Dots => dots(&rings, spacing, inset, pen_width, seed),
        FillStyle::Wave => wave(&rings, fill.angle_deg, spacing, inset),
        FillStyle::Trace => trace_fill(&rings, fill, inset, pen_width, seed),
    }
}

/// Relleno con el motor de trazos: la forma se convierte en un mapa de cobertura con el
/// tono elegido y se traza como una tinta de foto.
fn trace_fill(rings: &[Polyline], fill: &Fill, inset: f64, pen_width: f64, seed: u64) -> Vec<Polyline> {
    let prm = fill.trace_params();
    let b = BBox::of(rings.iter());
    if b.width() <= 0.0 || b.height() <= 0.0 {
        return vec![];
    }
    // Píxeles finos frente a la separación, sin pasar de 1600 por lado.
    let px = (prm.spacing_mm * 0.25).clamp(0.06, 0.5).max(b.width().max(b.height()) / 1600.0);
    let frame = raster::Frame::around(&b, px);
    let mask = raster::rasterize(rings, &frame);
    let tone = fill.tone.clamp(0.0, 1.0) as f32;
    let mut cov = raster::Grid::<f32>::new(frame.w, frame.h);
    if inset > 0.0 {
        let dist = raster::distance_inside(&mask);
        let min = (inset / px) as f32;
        for (c, d) in cov.data.iter_mut().zip(&dist.data) {
            if *d >= min.max(0.5) { *c = tone; }
        }
    } else {
        for (c, m) in cov.data.iter_mut().zip(&mask.data) {
            if *m { *c = tone; }
        }
    }
    crate::trace::render(&cov, px, &prm, pen_width, seed as usize)
        .into_iter()
        .map(|l| l.into_iter().map(|q| frame.origin + q).collect())
        .collect()
}

fn rotate_all(rings: &[Polyline], rad: f64) -> Vec<Polyline> {
    rings.iter().map(|r| r.iter().map(|q| q.rotate(rad)).collect()).collect()
}

/// Intervalos par/impar de la recta horizontal y = c.
fn spans(rings: &[Polyline], c: f64) -> Vec<(f64, f64)> {
    let mut xs = Vec::new();
    for ring in rings {
        let n = ring.len();
        if n < 3 || ring.iter().any(|q| !q.x.is_finite() || !q.y.is_finite()) { continue; }
        let ring_start = xs.len();
        let mut k = n - 1;
        for j in 0..n {
            let (a, z) = (ring[k], ring[j]);
            if (a.y > c) != (z.y > c) {
                let x = a.x + (c - a.y) * (z.x - a.x) / (z.y - a.y);
                if !x.is_finite() {
                    // Un contorno inválido no debe alterar la paridad de los demás.
                    xs.truncate(ring_start);
                    break;
                }
                xs.push(x);
            }
            k = j;
        }
    }
    xs.sort_by(f64::total_cmp);
    xs.chunks(2).filter(|w| w.len() == 2 && w[1] - w[0] > 1e-9).map(|w| (w[0], w[1])).collect()
}

/// Comprueba que el tramo a→b queda dentro de la forma (muestreo).
/// Los enlaces suelen ir pegados al contorno, así que se admite una tolerancia pequeña.
fn segment_inside(rings: &[Polyline], a: P, b: P, tol: f64) -> bool {
    (1..8).all(|i| {
        let q = a.lerp(b, i as f64 / 8.0);
        inside_rings(q, rings) || crate::geom::dist_to_lines(q, rings) <= tol
    })
}

fn hatch(rings: &[Polyline], angle_deg: f64, spacing: f64, inset: f64, connect: bool) -> Vec<Polyline> {
    let rad = angle_deg.to_radians();
    // Se gira la forma para que las líneas sean horizontales y se deshace al final.
    let local = rotate_all(rings, -rad);
    let b = BBox::of(local.iter());
    // Anclado a la hoja: formas vecinas comparten las mismas líneas.
    let mut c = (b.min.y / spacing).ceil() * spacing;
    let mut rows: Vec<(f64, Vec<(f64, f64)>)> = Vec::new();
    while c < b.max.y {
        let s: Vec<(f64, f64)> = spans(&local, c)
            .into_iter()
            .map(|(a, z)| (a + inset, z - inset))
            .filter(|(a, z)| z - a > spacing * 0.05)
            .collect();
        rows.push((c, s));
        c += spacing;
    }
    let back = |q: P| q.rotate(rad);
    let mut out = Vec::new();
    if !connect {
        for (i, (y, s)) in rows.iter().enumerate() {
            for &(a, z) in s {
                let (u, v) = if i % 2 == 0 { (a, z) } else { (z, a) };
                out.push(vec![back(p(u, *y)), back(p(v, *y))]);
            }
        }
        return out;
    }
    // Zigzag: encadena tramos de filas consecutivas mientras el enlace quede dentro.
    let mut used: Vec<Vec<bool>> = rows.iter().map(|(_, s)| vec![false; s.len()]).collect();
    for r0 in 0..rows.len() {
        for i0 in 0..rows[r0].1.len() {
            if used[r0][i0] {
                continue;
            }
            used[r0][i0] = true;
            let (y0, (a0, z0)) = (rows[r0].0, rows[r0].1[i0]);
            let mut chain = vec![p(a0, y0), p(z0, y0)];
            let mut left_to_right = true;
            let mut r = r0;
            let mut cur = (a0, z0);
            while r + 1 < rows.len() {
                let (yn, ref sn) = rows[r + 1];
                let end = *chain.last().unwrap();
                let mut found = None;
                for (j, &(a, z)) in sn.iter().enumerate() {
                    if used[r + 1][j] || z < cur.0 - spacing || a > cur.1 + spacing {
                        continue;
                    }
                    let start = if left_to_right { p(z, yn) } else { p(a, yn) };
                    if segment_inside(&local, end, start, spacing * 0.1) {
                        found = Some((j, a, z));
                        break;
                    }
                }
                let Some((j, a, z)) = found else { break };
                used[r + 1][j] = true;
                if left_to_right {
                    chain.push(p(z, yn));
                    chain.push(p(a, yn));
                } else {
                    chain.push(p(a, yn));
                    chain.push(p(z, yn));
                }
                left_to_right = !left_to_right;
                cur = (a, z);
                r += 1;
            }
            out.push(chain.into_iter().map(back).collect());
        }
    }
    out
}

fn concentric(rings: &[Polyline], spacing: f64, inset: f64) -> Vec<Polyline> {
    let b = BBox::of(rings.iter());
    // Resolución fina pero acotada para no disparar la memoria.
    let px = (spacing / 4.0).max(b.width().max(b.height()) / 1600.0).max(0.02);
    let frame = raster::Frame::around(&b, px);
    let mask = raster::rasterize(rings, &frame);
    let field = raster::distance_inside(&mask);
    let max_d = field.data.iter().cloned().fold(0.0f32, f32::max) as f64 * px;
    let mut out = Vec::new();
    let mut d = inset + spacing * 0.5;
    while d < max_d {
        for c in raster::contours(&field, (d / px) as f32) {
            let mm: Polyline = c.into_iter().map(|q| frame.to_mm(q)).collect();
            let s = simplify(&mm, px * 0.35);
            if s.len() >= 2 {
                out.push(s);
            }
        }
        d += spacing;
    }
    out
}

fn dots(rings: &[Polyline], spacing: f64, inset: f64, pen_width: f64, seed: u64) -> Vec<Polyline> {
    let b = BBox::of(rings.iter());
    let mut rng = Rng::new(seed ^ 0xD0_75);
    let r = (pen_width * 0.3).max(0.12);
    let row_h = spacing * 0.866;
    let mut out = Vec::new();
    let mut y = (b.min.y / row_h).ceil() * row_h;
    let mut row = 0usize;
    while y < b.max.y {
        let offset = if row.is_multiple_of(2) { 0.0 } else { spacing / 2.0 };
        let mut x = (b.min.x / spacing).floor() * spacing + offset;
        let mut line_dots = Vec::new();
        while x < b.max.x {
            let q = p(x + rng.sym() * spacing * 0.12, y + rng.sym() * spacing * 0.12);
            if inside_rings(q, rings) && (inset <= 0.0 || crate::geom::dist_to_lines(q, rings) >= inset) {
                line_dots.push((0..=6).map(|k| q + p(r, 0.0).rotate(k as f64 / 6.0 * std::f64::consts::TAU)).collect());
            }
            x += spacing;
        }
        if row % 2 == 1 {
            line_dots.reverse();
        }
        out.extend(line_dots);
        y += row_h;
        row += 1;
    }
    out
}

fn wave(rings: &[Polyline], angle_deg: f64, spacing: f64, inset: f64) -> Vec<Polyline> {
    let rad = angle_deg.to_radians();
    let local = rotate_all(rings, -rad);
    let b = BBox::of(local.iter());
    let step = (spacing / 5.0).max(0.05);
    let amp = spacing * 0.35;
    let period = spacing * 3.0;
    let mut out = Vec::new();
    let mut c = (b.min.y / spacing).ceil() * spacing;
    let mut row = 0usize;
    while c < b.max.y + amp {
        let mut current: Polyline = Vec::new();
        let mut row_lines = Vec::new();
        let mut x = b.min.x - step;
        while x <= b.max.x + step {
            let q = p(x, c + amp * (x / period * std::f64::consts::TAU).sin());
            let ok = inside_rings(q, &local) && (inset <= 0.0 || crate::geom::dist_to_lines(q, &local) >= inset);
            if ok {
                current.push(q.rotate(rad));
            } else if current.len() >= 2 {
                row_lines.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
            x += step;
        }
        if current.len() >= 2 {
            row_lines.push(current);
        }
        if row % 2 == 1 {
            row_lines.reverse();
            for l in &mut row_lines {
                l.reverse();
            }
        }
        out.extend(row_lines);
        c += spacing;
        row += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanline_ignores_empty_and_nonfinite_rings() {
        let good = square(10.0);
        let expected = spans(&good, 5.0);
        for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut rings = good.clone();
            rings.push(vec![]);
            rings.push(vec![p(x, 0.0), p(5.0, 10.0), p(0.0, 0.0)]);
            assert_eq!(spans(&rings, 5.0), expected);
        }
        let extreme = vec![vec![p(f64::MAX, -f64::MAX), p(-f64::MAX, f64::MAX), p(0.0, 0.0)]];
        assert!(spans(&extreme, 0.0).iter().all(|(a, b)| a.is_finite() && b.is_finite()));
        assert_eq!(spans(&[good[0].clone(), extreme[0].clone()], 5.0), expected);
    }

    fn square(s: f64) -> Vec<Polyline> {
        vec![vec![p(0.0, 0.0), p(s, 0.0), p(s, s), p(0.0, s)]]
    }

    #[test]
    fn hatch_square_has_expected_lines() {
        let f = Fill { style: FillStyle::Hatch, angle_deg: 0.0, spacing_mm: 1.0, ..Default::default() };
        let l = generate(&square(10.0), &f, 0.5, 1);
        assert!((9..=11).contains(&l.len()), "{}", l.len());
    }

    #[test]
    fn zigzag_square_is_one_stroke() {
        let f = Fill { style: FillStyle::Zigzag, angle_deg: 0.0, spacing_mm: 1.0, ..Default::default() };
        let l = generate(&square(10.0), &f, 0.5, 1);
        assert_eq!(l.len(), 1);
    }

    #[test]
    fn hole_is_left_empty() {
        let mut rings = square(10.0);
        rings.push(vec![p(3.0, 3.0), p(7.0, 3.0), p(7.0, 7.0), p(3.0, 7.0)]);
        let f = Fill { style: FillStyle::Hatch, angle_deg: 0.0, spacing_mm: 1.0, ..Default::default() };
        for l in generate(&rings, &f, 0.5, 1) {
            let mid = l[0].lerp(l[1], 0.5);
            assert!(!(mid.x > 3.0 && mid.x < 7.0 && mid.y > 3.0 && mid.y < 7.0));
        }
    }

    #[test]
    fn concentric_produces_rings() {
        let f = Fill { style: FillStyle::Concentric, spacing_mm: 1.0, ..Default::default() };
        let l = generate(&square(10.0), &f, 0.5, 1);
        assert!(l.len() >= 4);
    }

    #[test]
    fn trace_fill_stays_inside_the_shape_with_every_style_and_follows_the_tone() {
        use crate::trace::{TraceStyle, Vibe};
        // Cuadrado de 20 mm desplazado, con un agujero de 6 mm en el centro.
        let mut rings: Vec<Polyline> = vec![vec![p(30.0, 40.0), p(50.0, 40.0), p(50.0, 60.0), p(30.0, 60.0)]];
        rings.push(vec![p(37.0, 47.0), p(43.0, 47.0), p(43.0, 53.0), p(37.0, 53.0)]);
        let length = |l: &[Polyline]| l.iter().map(|l| crate::geom::length(l)).sum::<f64>();
        for style in TraceStyle::ALL {
            for vibe in [Vibe::None, Vibe::Tremor] {
                let f = Fill { style: FillStyle::Trace, spacing_mm: 1.0, trace: crate::trace::TraceParams { style, vibe, ..Default::default() }, ..Default::default() };
                let lines = generate(&rings, &f, 0.4, 3);
                assert!(!lines.is_empty(), "{style:?} {vibe:?} sin trazos");
                let tol = 0.45;
                for q in lines.iter().flatten() {
                    assert!(q.x > 30.0 - tol && q.x < 50.0 + tol && q.y > 40.0 - tol && q.y < 60.0 + tol, "{style:?} {vibe:?} fuera: {q:?}");
                    let hole = q.x > 37.0 + tol && q.x < 43.0 - tol && q.y > 47.0 + tol && q.y < 53.0 - tol;
                    assert!(!hole, "{style:?} {vibe:?} pisa el agujero: {q:?}");
                }
                if style == TraceStyle::Hatch && vibe == Vibe::None {
                    let light = generate(&rings, &Fill { tone: 0.3, ..f.clone() }, 0.4, 3);
                    assert!(length(&light) < length(&lines) * 0.6, "el tono bajo pone menos tinta");
                    let inset = generate(&rings, &Fill { inset_mm: 2.0, ..f.clone() }, 0.4, 3);
                    assert!(inset.iter().flatten().all(|q| q.x > 31.7 && q.x < 48.3), "respeta el margen");
                }
            }
        }
    }

    #[test]
    fn old_fills_open_with_full_tone_and_default_effects() {
        let old = r#"{"style":"Hatch","pen":1,"spacing_mm":1.6,"angle_deg":30.0,"inset_mm":0.0}"#;
        let f: Fill = serde_json::from_str(old).unwrap();
        assert_eq!((f.tone, f.trace.clone()), (1.0, crate::trace::TraceParams::default()));
        assert_eq!(f.trace_params().angle_deg, 30.0);
    }
}
