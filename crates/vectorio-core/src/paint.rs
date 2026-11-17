//! Pinceladas: una pintura de toques de color que se construye mirando el papel.
//!
//! Es el Boceto llevado al color. Simula la tinta de cada rotulador sobre el papel y
//! pone el siguiente toque donde, y con el color que, más acerca el papel a la foto:
//! - **Color físico.** Cada rotulador es un filtro transparente: su densidad óptica
//!   por canal (`-ln(color)`) se suma a la que ya hay. Así un amarillo sobre un cian
//!   da verde, como en el papel, y el motor sabe qué color le falta a cada zona.
//! - **Cada toque elige su color.** Gana el rotulador que más reduce la diferencia con
//!   la foto, castigando lo que se pasa (la tinta no se borra).
//! - **Color roto.** No siempre gana el mejor: entre los que casi empatan se elige al
//!   azar, y la zona se construye con toques de colores vecinos que se mezclan en el
//!   ojo, como en el divisionismo.
//! - **De lo grande a lo pequeño.** Primero manchas anchas y largas mirando la foto
//!   desenfocada, luego toques medios que siguen las formas y al final toques cortos,
//!   casi puntos, que afinan el detalle y los bordes.
//! - **Toques de pincel.** Cada toque es un gesto corto que se curva con la forma y se
//!   para al llegar a un color que no es el suyo. Con un pincel ancho se dibuja como un
//!   zigzag apretado de varias pasadas, con la punta afilada, sin levantar la pluma.
//! - **Viveza.** El color de la foto se satura antes de pintarlo.
//! - **Acentos**: trazos sueltos del color más oscuro en los bordes que lindan con
//!   una sombra.

use crate::analysis::{self, Analysis};
use crate::geom::{chaikin, length, p, Polyline, Rng, P};
use crate::raster::Grid;
use crate::sketch::{gauss, offset, value_noise, Focus};
use crate::vectorize::{kmeans, luma, to_u8, VecLayer, VecParams};
use image::RgbImage;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaintPalette {
    /// Once rotuladores de colores vivos.
    Vivid,
    /// Los colores principales de la foto, avivados.
    Photo,
    /// Los rotuladores del documento que se elijan (`VecParams::mix_pens`).
    Mine,
}

impl PaintPalette {
    pub const ALL: [PaintPalette; 3] = [PaintPalette::Vivid, PaintPalette::Photo, PaintPalette::Mine];
    pub fn label(self) -> &'static str {
        match self {
            PaintPalette::Vivid => "Colores vivos",
            PaintPalette::Photo => "Colores de la foto",
            PaintPalette::Mine => "Mis rotuladores",
        }
    }
}

/// Rotuladores de la paleta viva: del claro al oscuro, con un siena para los tonos
/// tierra y un índigo en lugar del negro para que las sombras sigan teniendo color.
pub const VIVID: [(&str, [u8; 3]); 11] = [
    ("Amarillo", [250, 220, 40]),
    ("Naranja", [246, 132, 34]),
    ("Rosa", [240, 120, 160]),
    ("Rojo", [214, 40, 52]),
    ("Magenta", [204, 36, 128]),
    ("Verde", [44, 156, 74]),
    ("Turquesa", [0, 160, 192]),
    ("Marrón", [146, 84, 44]),
    ("Violeta", [116, 62, 166]),
    ("Azul", [34, 70, 182]),
    ("Índigo", [40, 34, 80]),
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaintParams {
    pub palette: PaintPalette,
    /// Anchos de punta que caben en el lado mayor.
    pub density: u32,
    /// Tope de toques.
    pub max_strokes: u32,
    /// Exponente del tono: más alto aclara los medios tonos.
    pub tone: f64,
    /// Luces que se quedan en papel, 0–0.6.
    pub paper: f64,
    /// Saturación añadida al color de la foto, 0–1.
    pub vibrance: f64,
    /// Color roto, 0–1: con más, los toques vecinos alternan colores parecidos.
    pub broken: f64,
    /// Ancho del toque, en anchos de punta.
    pub brush: f64,
    /// Largo del toque medio, en anchos de punta.
    pub dab: f64,
    /// Cuánto siguen los toques las formas de la foto, 0 = todos en el ángulo base.
    pub follow: f64,
    /// Ángulo de los toques donde la foto no marca dirección.
    pub angle_deg: f64,
    /// Soltura de la mano, 0–1: toques en arco.
    pub hand: f64,
    /// Foco, 0–1: el centro de interés lleva todo el color y los bordes se disuelven.
    pub focus: f64,
    /// Acentos oscuros en los bordes, 0 = ninguno.
    pub accents: f64,
}

impl Default for PaintParams {
    fn default() -> Self {
        PaintParams {
            palette: PaintPalette::Vivid,
            density: 300,
            max_strokes: 16_000,
            tone: 1.1,
            paper: 0.2,
            vibrance: 0.35,
            broken: 0.5,
            brush: 2.5,
            dab: 8.0,
            follow: 0.7,
            angle_deg: 55.0,
            hand: 0.4,
            focus: 0.3,
            accents: 0.25,
        }
    }
}

/// Reflectancia mínima de una tinta: ningún rotulador llega al negro absoluto.
const MIN_R: f64 = 0.04;
/// Densidad óptica a la que tiende, sin llegar, lo que se pide por canal.
const MAX_OD: f32 = 2.6;
/// Líneas de un rotulador que debe pedir una zona para sembrar un toque.
const SEED: f32 = 0.7;
/// Por debajo de esto, la foto no quiere este color aquí: el toque se para.
const FOREIGN: f32 = 0.3;
/// Exigencia de parentesco entre el color del toque y el de la foto.
const KIN: i32 = 6;

fn od(c: [f64; 3]) -> [f32; 3] {
    c.map(|v| -(v.clamp(MIN_R, 1.0)).ln() as f32)
}

struct Pen {
    color: [u8; 3],
    name: Option<&'static str>,
    a: [f32; 3],
    norm2: f32,
    hue: [f32; 2],
}

/// Color oponente (rojo–verde, amarillo–azul) de una reflectancia 0–1.
fn opponent(c: [f32; 3]) -> [f32; 2] {
    [0.5 * (c[0] - c[1]), 0.5 * (0.5 * (c[0] + c[1]) - c[2])]
}

impl Pen {
    fn new(color: [u8; 3], name: Option<&'static str>) -> Pen {
        let a = od(color.map(|v| v as f64 / 255.0));
        Pen { color, name, a, norm2: a.iter().map(|v| v * v).sum::<f32>().max(1e-4), hue: opponent(color.map(|v| v as f32 / 255.0)) }
    }

    /// Cuántas pasadas de este rotulador pide un residuo (densidad que falta), por
    /// proyección. Lo que ya se ha pasado de un canal cuenta doble: no se borra.
    fn want(&self, r: [f32; 3]) -> f32 {
        let dot: f32 = (0..3).map(|c| self.a[c] * if r[c] < 0.0 { 2.0 * r[c] } else { r[c] }).sum();
        dot / self.norm2
    }

    /// Parentesco con el color que pide la foto (coseno entre densidades), afilado.
    /// En la media un toque casi negro oscurece bien un rojo, pero en el papel se ve como
    /// una mancha negra: las sombras se hacen con colores hermanos, no con el oscuro.
    fn kin(&self, target: [f32; 3]) -> f32 {
        let t2: f32 = target.iter().map(|v| v * v).sum();
        if t2 < 1e-6 {
            return 0.0;
        }
        let cos = (0..3).map(|c| self.a[c] * target[c]).sum::<f32>() / (self.norm2 * t2).sqrt();
        // El tono, en color oponente: en densidades un naranja y un amarillo se parecen
        // demasiado (los dos son casi solo azul), y a la vista no.
        let hue = opponent(target.map(|v| (-v).exp()));
        let (m, mp) = (hue[0].hypot(hue[1]), self.hue[0].hypot(self.hue[1]));
        let same = if m > 1e-4 && mp > 1e-4 { ((hue[0] * self.hue[0] + hue[1] * self.hue[1]) / (m * mp)).max(0.0) } else { 0.0 };
        // Cuanto más color tiene la zona, más cuenta el tono; en los grises, nada. En
        // las sombras hondas se relaja: un rojo oscuro admite violetas y azules.
        let c = target.map(|v| (-v).exp());
        let light = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        let weight = (m / 0.12).min(1.0) * (light / 0.22).min(1.0);
        cos.max(0.0).powi(KIN) * (1.0 - weight * (1.0 - same.powi(3)))
    }
}

pub fn render(rgb: &RgbImage, prm: &VecParams) -> Vec<VecLayer> {
    if rgb.width() < 16 || rgb.height() < 16 {
        return vec![];
    }
    let cfg = &prm.paint;
    let a = Analysis::new(rgb, &prm.analysis, prm.blur);
    let color = vivid_color(&a, cfg.vibrance);
    let pens = palette(&color, prm);
    if pens.is_empty() {
        return vec![];
    }
    let mut paint = Paint::new(&a, &color, pens, cfg);
    if cfg.accents > 0.01 {
        paint.accents(&a, cfg);
    }
    let base = cfg.dab.clamp(1.0, 60.0);
    let brush = cfg.brush.clamp(1.0, 8.0);
    let passes = [
        Pass { level: 0, fill: 0.6, length: base * 1.8, brush: brush * 1.4, step: 2.0, turn_deg: 8.0, follow: 0.6, detail: 0.0, share: 0.3, min_len: 4.0 },
        Pass { level: 1, fill: 0.9, length: base, brush, step: 1.5, turn_deg: 12.0, follow: 1.0, detail: 0.3, share: 0.4, min_len: 2.5 },
        Pass { level: 2, fill: 1.0, length: (base * 0.45).max(2.0), brush: (brush * 0.5).max(1.0), step: 1.0, turn_deg: 16.0, follow: 1.2, detail: 1.0, share: 0.3, min_len: 0.8 },
    ];
    let total = cfg.max_strokes.clamp(100, 80_000) as usize;
    for pass in &passes {
        let before = paint.strokes.len();
        paint.run(pass, cfg, (total as f64 * pass.share) as usize);
        if std::env::var("VECTORIO_DEBUG").is_ok() {
            let made = &paint.strokes[before..];
            eprintln!("pasada nivel {}: {} toques", pass.level, made.len());
        }
    }
    let to_px = 1.0 / paint.k;
    // Del más claro al más oscuro: el amarillo se pinta primero y el oscuro al final.
    let mut order: Vec<usize> = (0..paint.pens.len()).collect();
    order.sort_by(|&i, &j| luma(paint.pens[j].color).total_cmp(&luma(paint.pens[i].color)));
    let mut layers = Vec::new();
    for i in order {
        let lines: Vec<Polyline> = paint.strokes.iter().filter(|s| s.0 == i).map(|s| s.1.iter().map(|q| *q * to_px).collect()).collect();
        if !lines.is_empty() {
            let pen = &paint.pens[i];
            layers.push(VecLayer { name: pen.name, ..VecLayer::lines(pen.color, false, lines) });
        }
    }
    layers
}

/// Color de la foto ya suavizado y con contraste local (la luz y el color oponente del
/// análisis, de vuelta a RGB), con niveles automáticos y la saturación avivada.
fn vivid_color(a: &Analysis, vibrance: f64) -> Grid<[f32; 3]> {
    let mut sorted: Vec<f32> = a.light.data.iter().step_by(7).copied().collect();
    sorted.sort_by(f32::total_cmp);
    let at = |f: f64| sorted[((sorted.len() - 1) as f64 * f) as usize];
    // Las sombras se estiran hasta el fondo; las luces solo si la foto es apagada, para
    // no quemar los colores claros e intensos (un pétalo amarillo no es papel).
    let (lo, hi) = (at(0.01), at(0.99).max(0.85));
    let span = (hi - lo).max(0.35);
    let lo = lo.min(1.0 - span);
    let gain = 1.0 + 1.4 * vibrance.clamp(0.0, 1.0) as f32;
    let inverse = invert([[0.2126, 0.7152, 0.0722], [0.5, -0.5, 0.0], [0.25, 0.25, -0.5]]);
    let data = (0..a.light.data.len())
        .map(|i| {
            let l = ((a.light.data[i] - lo) / span).clamp(0.0, 1.0);
            let (c0, c1) = (a.chroma[0].data[i], a.chroma[1].data[i]);
            // Más ganancia en los colores apagados que en los ya intensos.
            let chroma = c0.hypot(c1);
            let g = 1.0 + (gain - 1.0) * (1.0 - (chroma / 0.35).min(1.0));
            let v = [l, c0 * g, c1 * g];
            let mut rgb = [0.0f32; 3];
            for (c, row) in rgb.iter_mut().zip(&inverse) {
                *c = row[0] * v[0] + row[1] * v[1] + row[2] * v[2];
            }
            // Un canal que se sale por arriba oscurece algo el color en vez de lavarlo:
            // así conserva el tono y la intensidad.
            let max = rgb.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            if max > 1.0 {
                rgb = rgb.map(|c| c - 0.7 * (max - 1.0));
            }
            rgb.map(|c| c.clamp(0.0, 1.0))
        })
        .collect();
    Grid { w: a.light.w, h: a.light.h, data }
}

fn invert(m: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let mut out = [[0.0f32; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            let (r0, r1) = ((j + 1) % 3, (j + 2) % 3);
            let (c0, c1) = ((i + 1) % 3, (i + 2) % 3);
            *v = (m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]) / det;
        }
    }
    out
}

fn palette(color: &Grid<[f32; 3]>, prm: &VecParams) -> Vec<Pen> {
    let vivid = || VIVID.iter().map(|(n, c)| Pen::new(*c, Some(n))).collect::<Vec<_>>();
    match prm.paint.palette {
        PaintPalette::Vivid => vivid(),
        PaintPalette::Mine => {
            let mut mine = prm.mix_pens.clone();
            mine.dedup();
            let pens: Vec<Pen> = mine.into_iter().filter(|c| luma(*c) < 240.0).map(|c| Pen::new(c, None)).collect();
            if pens.is_empty() { vivid() } else { pens }
        }
        PaintPalette::Photo => {
            let pixels: Vec<[u8; 3]> = color.data.iter().step_by(3).map(|c| c.map(|v| (v * 255.0).round() as u8)).collect();
            let mut pens: Vec<Pen> = kmeans(&pixels, prm.colors.clamp(2, 12), 7).into_iter().map(to_u8).filter(|c| luma(*c) < 235.0).map(|c| Pen::new(c, None)).collect();
            // Sin un color oscuro las sombras no llegan: se añade el índigo.
            if !pens.iter().any(|p| luma(p.color) < 70.0) {
                pens.push(Pen::new(VIVID[10].1, Some(VIVID[10].0)));
            }
            pens
        }
    }
}

struct Pass {
    level: usize,
    /// Parte del color final que se busca en esta pasada.
    fill: f32,
    /// Largo máximo del toque, en anchos de punta.
    length: f64,
    /// Ancho del toque, en anchos de punta.
    brush: f64,
    step: f64,
    turn_deg: f64,
    follow: f64,
    /// Cuánto se prefiere el detalle (bordes) al elegir dónde pintar.
    detail: f32,
    share: f64,
    /// Largo mínimo del gesto: en la última pasada caben casi puntos.
    min_len: f64,
}

/// Un nivel de la pirámide: densidad óptica pedida y puesta por canal.
struct Level {
    cell: f64,
    w: usize,
    h: usize,
    target: Vec<[f32; 3]>,
    ink: Vec<[f32; 3]>,
    importance: Vec<f32>,
    tried: Vec<f32>,
    jitter: Vec<f32>,
    /// Mejor valor de cada mosaico: (valor, celda, rotulador).
    tiles: Vec<(f32, usize, usize)>,
}

const TILE: usize = 8;
const RADIUS: f64 = 2.6;

impl Level {
    fn sample3(&self, v: &[[f32; 3]], q: P) -> [f32; 3] {
        let x = (q.x - 0.5).clamp(0.0, (self.w - 1) as f64);
        let y = (q.y - 0.5).clamp(0.0, (self.h - 1) as f64);
        let (ix, iy) = (x as usize, y as usize);
        let (jx, jy) = ((ix + 1).min(self.w - 1), (iy + 1).min(self.h - 1));
        let (tx, ty) = ((x - ix as f64) as f32, (y - iy as f64) as f32);
        let mut out = [0.0f32; 3];
        for (c, o) in out.iter_mut().enumerate() {
            let top = v[iy * self.w + ix][c] * (1.0 - tx) + v[iy * self.w + jx][c] * tx;
            let bottom = v[jy * self.w + ix][c] * (1.0 - tx) + v[jy * self.w + jx][c] * tx;
            *o = top * (1.0 - ty) + bottom * ty;
        }
        out
    }

    /// Lo que deja una pasada de un rotulador de densidad 1 en el centro de su raya.
    fn line(&self) -> f32 {
        gauss(0.0) as f32 / self.cell as f32
    }
}

struct Paint {
    k: f64,
    w: usize,
    h: usize,
    pens: Vec<Pen>,
    levels: Vec<Level>,
    field: Vec<(f32, f32)>,
    angle: Vec<f32>,
    strokes: Vec<(usize, Polyline)>,
    rng: Rng,
}

impl Paint {
    fn new(a: &Analysis, color: &Grid<[f32; 3]>, pens: Vec<Pen>, cfg: &PaintParams) -> Paint {
        let n = cfg.density.clamp(80, 1400) as usize;
        let k = n as f64 / a.w.max(a.h) as f64;
        let (w, h) = (((a.w as f64 * k).ceil() as usize).max(2), ((a.h as f64 * k).ceil() as usize).max(2));
        let sigma = (0.45 / k).max(0.0);
        let channels: Vec<Grid<f32>> = (0..3).map(|c| analysis::gaussian(&Grid { w: color.w, h: color.h, data: color.data.iter().map(|v| v[c]).collect() }, sigma)).collect();
        let paper = cfg.paper.clamp(0.0, 0.6) as f32 * 0.8;
        let tone = cfg.tone.clamp(0.3, 3.0) as f32;
        let focus = Focus::new(a, cfg.focus);
        let mut target = vec![[0.0f32; 3]; w * h];
        let mut important = Grid::<f32>::new(w, h);
        let mut field = vec![(0.0f32, 0.0f32); w * h];
        for y in 0..h {
            for x in 0..w {
                let q = p((x as f64 + 0.5) / k, (y as f64 + 0.5) / k);
                let rgb = [0, 1, 2].map(|c| analysis::sample(&channels[c], q) as f64);
                let t = od(rgb);
                // El papel: lo muy claro se queda sin pintar; al resto se le resta esa
                // luz por igual, así el tono no cambia.
                let max = t.iter().copied().fold(0.0f32, f32::max);
                let keep = if max > paper { (max - paper) / max } else { 0.0 };
                let shade = ((max - paper).max(0.0) / 2.0).min(1.5).powf(tone - 1.0).min(1.5);
                let f = keep * shade * focus.at(q);
                // Rodilla suave: un amarillo puro pide muchísima densidad en el azul, más
                // de lo que da un rotulador. Se escala el color entero, no cada canal, para
                // que conserve el tono y un rojo claro siga siendo más claro que uno oscuro.
                let t = t.map(|v| v * f);
                let top = t.iter().copied().fold(0.0f32, f32::max);
                let squeeze = if top > 1e-4 { MAX_OD * (top / MAX_OD).tanh() / top } else { 1.0 };
                target[y * w + x] = t.map(|v| v * squeeze);
                important.data[y * w + x] = analysis::sample(&a.importance, q);
                let (ix, iy) = ((q.x as usize).min(a.w - 1), (q.y as usize).min(a.h - 1));
                let (t, c) = a.tangent(ix, iy);
                field[y * w + x] = (t.y.atan2(t.x) as f32, c);
            }
        }
        let mut coherent: Vec<f32> = field.iter().map(|f| f.1).collect();
        coherent.sort_by(f32::total_cmp);
        let high = coherent[coherent.len() * 4 / 5].max(1e-4);
        for f in &mut field {
            f.1 = (f.1 / high).clamp(0.0, 1.0).powf(0.8);
        }
        let mut rng = Rng::new(41);
        let levels = [4.0, 2.0, 1.0].iter().map(|&cell| level(&target, &important, w, h, cell, &mut rng)).collect();
        // Los toques sin forma que seguir giran despacio, ±25°, como una mano que pinta.
        let base = cfg.angle_deg.to_radians() as f32;
        let drift = value_noise(w, h, 30.0, &mut rng);
        let angle = drift.iter().map(|v| base + v * 0.45).collect();
        Paint { k, w, h, pens, levels, field, angle, strokes: vec![], rng }
    }

    fn fine(&self) -> &Level {
        self.levels.last().unwrap()
    }

    fn darkest(&self) -> usize {
        (0..self.pens.len()).min_by(|&i, &j| luma(self.pens[i].color).total_cmp(&luma(self.pens[j].color))).unwrap()
    }

    /// Acentos: trazos sueltos del color más oscuro en los bordes que lindan con una
    /// sombra. Se pierden en las luces y se cortan a ratos, como hechos de memoria.
    fn accents(&mut self, a: &Analysis, cfg: &PaintParams) {
        let c = cfg.accents.clamp(0.0, 1.0);
        let ink = crate::ink::InkParams { scale: 1.6, sensitivity: 0.3 + 0.45 * c, continuity: 5.0, shadow_levels: 0, min_length: 16.0 + 30.0 * (1.0 - c) };
        let lines = crate::ink::contour_lines(a, &ink);
        let pen = self.darkest();
        let dab = cfg.dab.clamp(1.0, 60.0);
        let hand = cfg.hand.clamp(0.0, 1.0);
        for line in lines {
            let pts: Polyline = line.iter().map(|q| *q * self.k).collect();
            let total = length(&pts);
            let mut s = self.rng.f() * dab;
            while s < total {
                let len = dab * (0.9 + 0.9 * self.rng.f());
                let gap = dab * (0.3 + 0.6 * self.rng.f()) * (1.0 + 2.0 * (1.0 - c));
                let piece = cut(&pts, s, (s + len).min(total));
                s += len + gap;
                if piece.len() < 2 || length(&piece) < 2.0 || !self.dark_side(&piece) {
                    continue;
                }
                let mark = self.hand_line(&piece, hand);
                self.deposit(&mark, pen, 0.9);
                self.strokes.push((pen, mark));
            }
        }
    }

    /// ¿Linda el tramo con una zona oscura?
    fn dark_side(&self, pts: &[P]) -> bool {
        let lvl = self.fine();
        let mid = pts.len() / 2;
        let (a, b) = (pts[mid.saturating_sub(1)], pts[(mid + 1).min(pts.len() - 1)]);
        let d = b - a;
        let n = d.len().max(1e-9);
        let normal = p(-d.y / n, d.x / n) * 2.0;
        // Luz del color pedido: solo cuentan las sombras de verdad, no un rojo intenso.
        let light = |q: P| {
            let t = lvl.sample3(&lvl.target, q);
            0.2126 * (-t[0]).exp() + 0.7152 * (-t[1]).exp() + 0.0722 * (-t[2]).exp()
        };
        light(pts[mid] + normal).min(light(pts[mid] - normal)) < 0.16
    }

    fn residual(&self, lvl: &Level, i: usize, pass: &Pass) -> [f32; 3] {
        [0, 1, 2].map(|c| pass.fill * lvl.target[i][c] - lvl.ink[i][c])
    }

    fn residual_at(&self, lvl: &Level, q: P, pass: &Pass) -> [f32; 3] {
        let (t, ink) = (lvl.sample3(&lvl.target, q), lvl.sample3(&lvl.ink, q));
        [0, 1, 2].map(|c| pass.fill * t[c] - ink[c])
    }

    /// Valor de pintar con un rotulador en una celda: lo que se reduce la diferencia
    /// con la foto, con el detalle y los intentos fallidos. Devuelve también cuántas
    /// pasadas pide.
    fn value(&self, lvl: &Level, i: usize, pen: usize, pass: &Pass) -> (f32, f32) {
        let pen_ = &self.pens[pen];
        let s = pen_.want(self.residual(lvl, i, pass)) / lvl.line() - lvl.tried[i];
        let weight = 1.0 - pass.detail * 0.6 + pass.detail * 1.2 * lvl.importance[i];
        (pen_.norm2 * (2.0 * s - 1.0) * weight * pen_.kin(lvl.target[i]), s)
    }

    fn run(&mut self, pass: &Pass, cfg: &PaintParams, budget: usize) {
        self.levels[pass.level].tried.iter_mut().for_each(|v| *v = 0.0);
        self.refresh(pass, None);
        let mut made = 0;
        let mut misses = 0;
        let broken = cfg.broken.clamp(0.0, 1.0) as f32;
        while made < budget && misses < 3000 {
            let lvl = &self.levels[pass.level];
            let Some(&(value, seed, _)) = lvl.tiles.iter().max_by(|a, b| a.0.total_cmp(&b.0)) else { break };
            if value <= 0.0 {
                break;
            }
            // El color: entre los que casi empatan con el mejor, uno al azar según lo que
            // aporta. Así una zona se construye con toques de colores vecinos.
            let options: Vec<(usize, f32, f32)> = (0..self.pens.len()).map(|k| { let (v, s) = self.value(lvl, seed, k, pass); (k, v, s) }).collect();
            let best = options.iter().map(|o| o.1).fold(f32::NEG_INFINITY, f32::max);
            let floor = best * (1.0 - 0.55 * broken);
            let pool: Vec<&(usize, f32, f32)> = options.iter().filter(|o| o.1 >= floor && o.1 > 0.0 && o.2 >= SEED).collect();
            let start = p((seed % lvl.w) as f64 + 0.5, (seed / lvl.w) as f64 + 0.5) * lvl.cell;
            let pen = if pool.is_empty() {
                None
            } else {
                let sum: f32 = pool.iter().map(|o| o.1).sum();
                let mut pick = self.rng.f() as f32 * sum;
                let mut chosen = pool[0].0;
                for o in &pool {
                    chosen = o.0;
                    pick -= o.1;
                    if pick <= 0.0 {
                        break;
                    }
                }
                Some(chosen)
            };
            // La mancha grande mira la foto desenfocada; aun así no nace en una zona que
            // de cerca no quiere este color.
            let pen = pen.filter(|&k| self.own(start, k) >= FOREIGN);
            let line = pen.map(|k| self.gesture(start, k, pass, cfg)).unwrap_or_default();
            let Some(pen) = pen.filter(|_| line.len() >= 2 && length(&line) >= pass.min_len) else {
                misses += 1;
                let lvl = &mut self.levels[pass.level];
                lvl.tried[seed] += 0.6;
                let (x, y) = (seed % lvl.w, seed / lvl.w);
                self.refresh(pass, Some((x, y, x, y)));
                continue;
            };
            let mark = self.brush_mark(&line, pass.brush, cfg.hand);
            let bounds = self.deposit(&mark, pen, 1.0)[pass.level];
            self.strokes.push((pen, mark));
            if bounds.0 != usize::MAX {
                self.refresh(pass, Some(bounds));
            }
            made += 1;
        }
    }

    fn refresh(&mut self, pass: &Pass, bounds: Option<(usize, usize, usize, usize)>) {
        let lvl = &self.levels[pass.level];
        let (tw, th) = (lvl.w.div_ceil(TILE), lvl.h.div_ceil(TILE));
        let (tx0, ty0, tx1, ty1) = match bounds {
            Some((x0, y0, x1, y1)) => (x0 / TILE, y0 / TILE, (x1 / TILE).min(tw - 1), (y1 / TILE).min(th - 1)),
            None => (0, 0, tw - 1, th - 1),
        };
        let mut updates = Vec::new();
        for ty in ty0..=ty1 {
            for tx in tx0..=tx1 {
                let mut best = (f32::NEG_INFINITY, 0usize, 0usize);
                for y in ty * TILE..((ty + 1) * TILE).min(lvl.h) {
                    for x in tx * TILE..((tx + 1) * TILE).min(lvl.w) {
                        let i = y * lvl.w + x;
                        for k in 0..self.pens.len() {
                            let (v, s) = self.value(lvl, i, k, pass);
                            let v = v + lvl.jitter[i] * 0.05;
                            if s >= SEED && v > best.0 {
                                best = (v, i, k);
                            }
                        }
                    }
                }
                updates.push((ty * tw + tx, best));
            }
        }
        let lvl = &mut self.levels[pass.level];
        for (t, best) in updates {
            lvl.tiles[t] = best;
        }
    }

    /// Pasadas de un rotulador que pide la foto de cerca, sin contar lo ya pintado.
    fn own(&self, q: P, pen: usize) -> f32 {
        let fine = self.fine();
        self.pens[pen].want(fine.sample3(&fine.target, q)) / fine.line()
    }

    fn score(&self, q: P, theta: f64, pen: usize, pass: &Pass, cfg: &PaintParams) -> (f64, f32, f32) {
        let lvl = &self.levels[pass.level];
        let pen_ = &self.pens[pen];
        let dir = p(theta.cos(), theta.sin());
        let (mut sum, mut first, mut own) = (0.0f32, 0.0f32, 0.0f32);
        for s in 1..=4 {
            let at = q + dir * (pass.step * 0.5 * s as f64);
            if at.x < 0.0 || at.y < 0.0 || at.x >= self.w as f64 || at.y >= self.h as f64 {
                return (f64::NEG_INFINITY, 0.0, 0.0);
            }
            let r = pen_.want(self.residual_at(lvl, at * (1.0 / lvl.cell), pass)) / lvl.line();
            sum += r;
            if s <= 2 {
                first += r * 0.5;
                // ¿Quiere la foto este color aquí, de cerca? Un toque rojo no entra en
                // una hoja verde.
                own += self.own(at, pen) * 0.5;
            }
        }
        let tone = (sum / 4.0).clamp(-2.0, 3.0) as f64;
        let (x, y) = ((q.x as usize).min(self.w - 1), (q.y as usize).min(self.h - 1));
        let (angle, coherence) = self.field[y * self.w + x];
        let follow = (cfg.follow.clamp(0.0, 1.0) * pass.follow).min(1.0) * coherence as f64;
        let along = (theta - angle as f64).cos().abs().powi(6);
        let ruled = (theta - self.angle[y * self.w + x] as f64).cos().abs().powi(6);
        let aligned = follow * along + (1.0 - follow) * ruled;
        let foreign = ((own - FOREIGN) / FOREIGN).min(0.0) as f64;
        (tone + 2.0 * aligned + 1.5 * foreign, first, own)
    }

    fn gesture(&mut self, start: P, pen: usize, pass: &Pass, cfg: &PaintParams) -> Polyline {
        let mut best = (f64::NEG_INFINITY, 0.0);
        for i in 0..18 {
            let theta = i as f64 * PI / 18.0 + self.rng.f() * 0.05;
            let (a, _, _) = self.score(start, theta, pen, pass, cfg);
            let (b, _, _) = self.score(start, theta + PI, pen, pass, cfg);
            let s = a.max(-5.0) + b.max(-5.0);
            if s > best.0 {
                best = (s, theta);
            }
        }
        let half = 0.5 * pass.length * (0.6 + 0.4 * self.rng.f());
        let mut forward = self.grow(start, best.1, half, pen, pass, cfg);
        let backward = self.grow(start, best.1 + PI, half, pen, pass, cfg);
        let mut line: Polyline = backward.into_iter().rev().collect();
        line.pop();
        line.append(&mut forward);
        line
    }

    fn grow(&self, start: P, theta: f64, max_len: f64, pen: usize, pass: &Pass, cfg: &PaintParams) -> Polyline {
        let mut pts = vec![start];
        let mut q = start;
        let mut heading = theta;
        let mut travelled = 0.0;
        let turn = pass.turn_deg.to_radians();
        let keep = 0.35f32;
        let sure = max_len * 0.4;
        let mut mean = keep * 2.0;
        let mut turning = 0.0;
        let mut foreign = 0;
        while travelled < max_len {
            let mut best = (f64::NEG_INFINITY, heading, 0.0f32, 0.0f32);
            for j in -3i32..=3 {
                let t = heading + turn * j as f64 / 3.0;
                let (s, first, own) = self.score(q, t, pen, pass, cfg);
                let s = s - 0.15 * (j as f64 / 3.0).powi(2);
                if s > best.0 {
                    best = (s, t, first, own);
                }
            }
            mean = 0.7 * mean + 0.3 * best.2;
            foreign = if best.3 < FOREIGN { foreign + 1 } else { 0 };
            if best.0 == f64::NEG_INFINITY || foreign >= 2 || (mean < keep && travelled >= sure) || mean < keep * 0.25 {
                break;
            }
            turning = 0.6 * turning + 0.4 * (best.1 - heading);
            heading += turning;
            q = q + p(heading.cos(), heading.sin()) * pass.step;
            pts.push(q);
            travelled += pass.step;
        }
        pts
    }

    /// El toque de pincel: el gesto, suavizado y arqueado por la mano, se recorre en
    /// varias pasadas paralelas de ida y vuelta, sin levantar la pluma. Las pasadas de
    /// fuera son más cortas, así el toque se afila en las puntas, y la cola se adelgaza
    /// más que la cabeza, como al soltar el pincel.
    fn brush_mark(&mut self, line: &[P], width: f64, hand: f64) -> Polyline {
        let center = self.hand_line(&chaikin(line, 2, false), hand);
        let total = length(&center);
        let passes = ((width / 0.85).round() as usize).max(1);
        if passes == 1 || total < 0.5 {
            return center;
        }
        let spacing = width / passes as f64;
        let mut out: Polyline = Vec::new();
        for j in 0..passes {
            let o = (j as f64 - (passes - 1) as f64 / 2.0) * spacing;
            let edge = (o.abs() / (width / 2.0)).min(1.0);
            let head = total * (0.05 + 0.25 * edge * edge) * (0.8 + 0.4 * self.rng.f());
            let tail = total * (0.08 + 0.4 * edge * edge) * (0.8 + 0.4 * self.rng.f());
            if head + tail > total * 0.9 {
                continue;
            }
            let mut piece = cut(&offset(&center, o), head, total - tail);
            if piece.len() < 2 {
                continue;
            }
            if j % 2 == 1 {
                piece.reverse();
            }
            out.extend(piece);
        }
        if out.len() < 2 { center } else { out }
    }

    fn hand_line(&mut self, line: &[P], hand: f64) -> Polyline {
        let hand = hand.clamp(0.0, 1.0);
        let total = length(line);
        if hand < 1e-3 || line.len() < 3 || total < 1e-6 {
            return line.to_vec();
        }
        // Un toque es corto: se arquea, no tiembla.
        let bow = hand * 0.12 * total.min(40.0) * self.rng.sym();
        let mut s = 0.0;
        let mut out = Vec::with_capacity(line.len());
        for i in 0..line.len() {
            if i > 0 {
                s += line[i].dist(line[i - 1]);
            }
            let (a, b) = (line[i.saturating_sub(1)], line[(i + 1).min(line.len() - 1)]);
            let d = b - a;
            let n = d.len();
            if n < 1e-9 {
                out.push(line[i]);
                continue;
            }
            let t = s / total;
            out.push(line[i] + p(-d.y / n, d.x / n) * (bow * 4.0 * t * (1.0 - t)));
        }
        out
    }

    /// Pone la tinta de un trazo en todos los niveles; devuelve la caja tocada en cada
    /// uno.
    fn deposit(&mut self, line: &[P], pen: usize, strength: f32) -> Vec<(usize, usize, usize, usize)> {
        let a = self.pens[pen].a.map(|v| v * strength);
        let mut boxes = Vec::new();
        for lvl in &mut self.levels {
            let scale = 1.0 / lvl.cell;
            let amount = 1.0 / lvl.cell as f32;
            let mut bounds = (usize::MAX, usize::MAX, 0usize, 0usize);
            for pair in line.windows(2) {
                let (p0, p1) = (pair[0] * scale, pair[1] * scale);
                let x0 = ((p0.x.min(p1.x) - RADIUS).floor().max(0.0)) as usize;
                let y0 = ((p0.y.min(p1.y) - RADIUS).floor().max(0.0)) as usize;
                let x1 = ((p0.x.max(p1.x) + RADIUS).ceil() as usize).min(lvl.w - 1);
                let y1 = ((p0.y.max(p1.y) + RADIUS).ceil() as usize).min(lvl.h - 1);
                if x0 > x1 || y0 > y1 {
                    continue;
                }
                bounds = (bounds.0.min(x0), bounds.1.min(y0), bounds.2.max(x1), bounds.3.max(y1));
                let d = p1 - p0;
                let len2 = d.dot(d);
                if len2 < 1e-12 {
                    continue;
                }
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        let c = p(x as f64 + 0.5, y as f64 + 0.5);
                        let t = (c - p0).dot(d) / len2;
                        if !(0.0..1.0).contains(&t) {
                            continue;
                        }
                        let dist = (c - (p0 + d * t)).len();
                        if dist > RADIUS {
                            continue;
                        }
                        let add = amount * gauss(dist) as f32;
                        let cell = &mut lvl.ink[y * lvl.w + x];
                        for ch in 0..3 {
                            cell[ch] += add * a[ch];
                        }
                    }
                }
            }
            boxes.push(bounds);
        }
        boxes
    }
}

/// Tramo de una polilínea entre dos distancias desde su inicio.
fn cut(line: &[P], from: f64, to: f64) -> Polyline {
    let mut out = Vec::new();
    let mut s = 0.0;
    for pair in line.windows(2) {
        let seg = pair[0].dist(pair[1]);
        let (a, b) = (s, s + seg);
        if b >= from && a <= to && seg > 1e-12 {
            let t0 = ((from - a) / seg).clamp(0.0, 1.0);
            let t1 = ((to - a) / seg).clamp(0.0, 1.0);
            let q0 = pair[0].lerp(pair[1], t0);
            if out.last().is_none_or(|l: &P| l.dist(q0) > 1e-9) {
                out.push(q0);
            }
            out.push(pair[0].lerp(pair[1], t1));
        }
        s = b;
        if s > to {
            break;
        }
    }
    out
}

fn level(target: &[[f32; 3]], important: &Grid<f32>, w0: usize, h0: usize, cell: f64, rng: &mut Rng) -> Level {
    let (w, h) = (((w0 as f64 / cell).ceil() as usize).max(1), ((h0 as f64 / cell).ceil() as usize).max(1));
    let down = |get: &dyn Fn(usize) -> f32| {
        let mut out = Grid::<f32>::new(w, h);
        let mut count = vec![0.0f32; w * h];
        for y in 0..h0 {
            for x in 0..w0 {
                let (cx, cy) = (((x as f64 / cell) as usize).min(w - 1), ((y as f64 / cell) as usize).min(h - 1));
                out.data[cy * w + cx] += get(y * w0 + x);
                count[cy * w + cx] += 1.0;
            }
        }
        for (v, n) in out.data.iter_mut().zip(count) {
            *v /= n.max(1.0);
        }
        analysis::gaussian(&out, 1.0).data
    };
    let channels: Vec<Vec<f32>> = (0..3).map(|c| down(&|i| target[i][c])).collect();
    let target = (0..w * h).map(|i| [channels[0][i], channels[1][i], channels[2][i]]).collect();
    let importance = down(&|i| important.data[i]);
    let jitter = (0..w * h).map(|_| rng.f() as f32).collect();
    let tiles = vec![(f32::NEG_INFINITY, 0, 0); w.div_ceil(TILE) * h.div_ceil(TILE)];
    Level { cell, w, h, ink: vec![[0.0; 3]; w * h], tried: vec![0.0; w * h], target, importance, jitter, tiles }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectorize::{vectorize, VecMode};
    use image::DynamicImage;

    fn run(img: RgbImage, paint: PaintParams) -> Vec<VecLayer> {
        let prm = VecParams { mode: VecMode::Paint, max_px: 240, paint, ..Default::default() };
        vectorize(&DynamicImage::ImageRgb8(img), &prm).layers
    }

    fn ink(layers: &[VecLayer], color: [u8; 3], x0: f64, x1: f64) -> f64 {
        layers
            .iter()
            .filter(|l| l.color == color)
            .flat_map(|l| &l.lines)
            .flat_map(|l| l.windows(2))
            .filter(|s| (s[0].x + s[1].x) / 2.0 >= x0 && (s[0].x + s[1].x) / 2.0 < x1)
            .map(|s| (s[1] - s[0]).len())
            .sum()
    }

    fn pen(name: &str) -> [u8; 3] {
        VIVID.iter().find(|p| p.0 == name).unwrap().1
    }

    #[test]
    fn each_area_is_painted_with_its_own_colours() {
        // Rojo a la izquierda, azul a la derecha.
        let img = RgbImage::from_fn(240, 160, |x, _| image::Rgb(if x < 120 { [210, 40, 50] } else { [40, 70, 180] }));
        let layers = run(img, PaintParams { focus: 0.0, accents: 0.0, ..Default::default() });
        let (red, blue) = (pen("Rojo"), pen("Azul"));
        assert!(ink(&layers, red, 0.0, 110.0) > ink(&layers, red, 130.0, 240.0) * 4.0, "el rojo va a la izquierda");
        assert!(ink(&layers, blue, 130.0, 240.0) > ink(&layers, blue, 0.0, 110.0) * 4.0, "el azul va a la derecha");
    }

    #[test]
    fn white_stays_paper_and_light_colours_go_first() {
        let img = RgbImage::from_fn(240, 160, |x, _| image::Rgb(if x < 120 { [250, 250, 250] } else { [60, 150, 70] }));
        let layers = run(img, PaintParams { focus: 0.0, accents: 0.0, ..Default::default() });
        let all = |a, b| layers.iter().map(|l| ink(std::slice::from_ref(l), l.color, a, b)).sum::<f64>();
        assert!(all(0.0, 100.0) < all(140.0, 240.0) * 0.05, "el blanco queda en papel");
        let lumas: Vec<f64> = layers.iter().map(|l| luma(l.color)).collect();
        assert!(lumas.windows(2).all(|w| w[0] >= w[1]), "del claro al oscuro: {lumas:?}");
    }

    #[test]
    fn strokes_are_short_dabs_between_dots_and_lines() {
        let img = RgbImage::from_fn(240, 160, |x, y| image::Rgb([(x + y) as u8, 120, 255 - x as u8]));
        let layers = run(img, PaintParams { accents: 0.0, ..Default::default() });
        let lines: Vec<&Polyline> = layers.iter().flat_map(|l| &l.lines).collect();
        assert!(lines.len() > 100, "{} toques", lines.len());
        assert!(layers.len() >= 3, "varios colores: {}", layers.len());
    }

    #[test]
    fn broken_colour_mixes_more_pens_in_one_area() {
        let img = RgbImage::from_pixel(240, 160, image::Rgb([200, 110, 60]));
        let count = |broken| run(img.clone(), PaintParams { broken, focus: 0.0, accents: 0.0, ..Default::default() }).len();
        assert!(count(1.0) >= count(0.0), "con color roto {} tintas, sin él {}", count(1.0), count(0.0));
    }

    #[test]
    fn the_same_photo_gives_the_same_painting() {
        let img = RgbImage::from_fn(120, 90, |x, y| image::Rgb([((x * y) % 255) as u8, (x * 2) as u8, (y * 2) as u8]));
        let a = run(img.clone(), PaintParams::default());
        let b = run(img, PaintParams::default());
        assert_eq!(a.iter().map(|l| l.lines.clone()).collect::<Vec<_>>(), b.iter().map(|l| l.lines.clone()).collect::<Vec<_>>());
    }
}
