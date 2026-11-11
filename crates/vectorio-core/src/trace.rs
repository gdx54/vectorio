//! Motor de tramas: convierte un mapa de cobertura (cuánta tinta va en cada punto, de
//! 0 a 1) en recorridos de rotulador. Cada tinta de una foto viva guarda su mapa y sus
//! ajustes ([`Traced`]); los trazos se recalculan cuando cambian los ajustes o el
//! grosor del rotulador, así que el dibujo se puede retocar después de importarlo.

use crate::geom::{p, Polyline, P};
use crate::raster::Grid;
use crate::shared::{base64_decode, base64_encode, Shared};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceStyle {
    /// Rayado: líneas paralelas en el ángulo de cada tinta; la cobertura decide qué
    /// tramos se dibujan.
    Hatch,
    /// Flujo: líneas que siguen las formas del mapa (paralelas a sus bordes) y se
    /// detienen donde se acaba el color.
    Flow,
    /// Puntillismo: rejilla hexagonal con umbrales intercalados.
    Dots,
    /// Contornos: curvas de nivel paralelas al borde de cada mancha, hacia dentro.
    Contour,
    /// Espiral: una sola espiral por tinta que se ondula más donde hay más color.
    Spiral,
    /// Garabato: un trazo continuo que se enreda más donde hay más color.
    Scribble,
}

impl TraceStyle {
    pub const ALL: [TraceStyle; 6] = [TraceStyle::Hatch, TraceStyle::Flow, TraceStyle::Dots, TraceStyle::Contour, TraceStyle::Spiral, TraceStyle::Scribble];
    pub fn label(self) -> &'static str {
        match self {
            TraceStyle::Hatch => "Rayado",
            TraceStyle::Flow => "Flujo",
            TraceStyle::Dots => "Puntos",
            TraceStyle::Contour => "Contornos",
            TraceStyle::Spiral => "Espiral",
            TraceStyle::Scribble => "Garabato",
        }
    }
    pub fn help(self) -> &'static str {
        match self {
            TraceStyle::Hatch => "Líneas paralelas: más tramos donde hay más color.",
            TraceStyle::Flow => "Líneas que siguen las formas del dibujo sin salirse de su color.",
            TraceStyle::Dots => "Puntos en rejilla: más puntos donde hay más color.",
            TraceStyle::Contour => "Curvas de nivel que rodean cada mancha de color hacia dentro.",
            TraceStyle::Spiral => "Una sola espiral que se ondula más donde hay más color. Con varias tintas, cada centro se desplaza y aparece moiré.",
            TraceStyle::Scribble => "Un trazo continuo que se enreda más donde hay más color.",
        }
    }
    /// ¿Tiene sentido el ángulo en este estilo? (En Flujo manda en las zonas lisas.)
    pub fn uses_angle(self) -> bool {
        matches!(self, TraceStyle::Hatch | TraceStyle::Flow)
    }
    /// ¿Se trocea en tramos según el tono (largo y variación tienen sentido)?
    pub fn uses_dashes(self) -> bool {
        matches!(self, TraceStyle::Hatch | TraceStyle::Flow | TraceStyle::Contour | TraceStyle::Spiral)
    }
    /// ¿Admite vibración? (Los puntos no son líneas y la espiral ya se ondula sola.)
    pub fn uses_vibe(self) -> bool {
        !matches!(self, TraceStyle::Dots | TraceStyle::Spiral)
    }
}

/// Forma de la vibración que se añade a las líneas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vibe {
    #[default]
    None,
    /// Onda senoidal suave.
    Wave,
    /// Dientes de sierra en ángulo.
    Zigzag,
    /// Temblor irregular, como a mano alzada.
    Tremor,
}

impl Vibe {
    pub const ALL: [Vibe; 4] = [Vibe::None, Vibe::Wave, Vibe::Zigzag, Vibe::Tremor];
    pub fn label(self) -> &'static str {
        match self {
            Vibe::None => "Ninguna",
            Vibe::Wave => "Onda",
            Vibe::Zigzag => "Zigzag",
            Vibe::Tremor => "Temblor",
        }
    }
}

/// Cómo se traza una tinta. Los campos que faltan en archivos antiguos toman el valor
/// por defecto.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TraceParams {
    pub style: TraceStyle,
    /// Separación entre líneas o puntos con cobertura completa, mm.
    pub spacing_mm: f64,
    /// Dirección de las líneas, grados.
    pub angle_deg: f64,
    /// Largo de cada tramo en los tonos claros, mm (en los oscuros se juntan).
    pub dash_mm: f64,
    /// Cuánto cambia el largo de un tramo a otro (0–1).
    pub dash_vary: f64,
    /// Repasos extra donde hay más color, para que el trazo engorde (0–1).
    pub weight: f64,
    pub vibe: Vibe,
    /// Amplitud de la vibración, mm.
    pub vibe_mm: f64,
    /// Largo de onda de la vibración, mm.
    pub wave_mm: f64,
    /// La vibración crece con el color y se calma en los claros.
    pub vibe_by_tone: bool,
    /// Desfase de la trama en fracciones de la separación (0–1). Al entrelazar tintas,
    /// cada una ocupa su hueco entre las líneas de las demás.
    pub phase: f64,
    /// Relevo: las tintas de la foto que lo tienen comparten las mismas líneas y se
    /// pasan el trazo según el color que hay debajo.
    pub relay: bool,
}

impl Default for TraceParams {
    fn default() -> Self {
        TraceParams {
            style: TraceStyle::Hatch,
            spacing_mm: 0.9,
            angle_deg: 15.0,
            dash_mm: 3.6,
            dash_vary: 0.0,
            weight: 0.0,
            vibe: Vibe::None,
            vibe_mm: 0.5,
            wave_mm: 2.5,
            vibe_by_tone: true,
            phase: 0.0,
            relay: false,
        }
    }
}

impl TraceParams {
    /// Adopta el aspecto de otro ajuste (un estilo guardado) sin perder el ángulo ni el
    /// desfase propios de esta tinta, que son los que la combinan con las demás.
    pub fn adopt(&mut self, look: &TraceParams) {
        *self = TraceParams { angle_deg: self.angle_deg, phase: self.phase, relay: self.relay, ..look.clone() };
    }
}

/// Mezcla cruzada: cada tinta con su ángulo, repartidos en 180°, como en una cuatricromía.
pub fn mix_cross(inks: &mut [TraceParams]) {
    let n = inks.len().max(1) as f64;
    let a0 = inks.first().map(|t| t.angle_deg).unwrap_or(0.0);
    for (i, t) in inks.iter_mut().enumerate() {
        t.angle_deg = (a0 + 180.0 * i as f64 / n).rem_euclid(180.0);
        t.phase = 0.0;
        t.relay = false;
    }
}

/// Mezcla entrelazada: todas las tintas con el mismo estilo y ángulo, y cada una en su
/// hueco (A, B, C, A, B, C…): las líneas se alternan sin pisarse.
pub fn mix_interleave(inks: &mut [TraceParams]) {
    let n = inks.len().max(1) as f64;
    let Some(first) = inks.first().cloned() else { return };
    for (i, t) in inks.iter_mut().enumerate() {
        t.style = first.style;
        t.angle_deg = first.angle_deg;
        t.phase = i as f64 / n;
        t.relay = false;
    }
}

/// Relevo: todas las tintas con el mismo trazo que la primera. Comparten las líneas y
/// cada tramo lo pinta una u otra según el color que hay debajo.
pub fn mix_relay(inks: &mut [TraceParams]) {
    let Some(first) = inks.first().cloned() else { return };
    for t in inks.iter_mut() {
        *t = TraceParams { phase: 0.0, relay: true, ..first.clone() };
    }
}

/// Mapa de cobertura de una tinta y su tamaño en el papel.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(into = "CoverageFile", try_from = "CoverageFile")]
pub struct Coverage {
    pub map: Grid<f32>,
    pub mm_per_px: f64,
}

impl Coverage {
    pub fn size_mm(&self) -> (f64, f64) {
        (self.map.w as f64 * self.mm_per_px, self.map.h as f64 * self.mm_per_px)
    }
}

impl PartialEq for Coverage {
    fn eq(&self, other: &Self) -> bool {
        self.mm_per_px == other.mm_per_px && self.map.w == other.map.w && self.map.h == other.map.h && self.map.data == other.map.data
    }
}

/// En el archivo, el mapa va como PNG en escala de grises (256 niveles bastan).
#[derive(Serialize, Deserialize)]
struct CoverageFile {
    w: usize,
    h: usize,
    mm_per_px: f64,
    png: String,
}

impl From<Coverage> for CoverageFile {
    fn from(c: Coverage) -> Self {
        let pixels: Vec<u8> = c.map.data.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect();
        let mut png = Vec::new();
        if let Some(img) = image::GrayImage::from_raw(c.map.w as u32, c.map.h as u32, pixels) {
            let _ = img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png);
        }
        CoverageFile { w: c.map.w, h: c.map.h, mm_per_px: c.mm_per_px, png: base64_encode(&png) }
    }
}

impl TryFrom<CoverageFile> for Coverage {
    type Error = String;
    fn try_from(f: CoverageFile) -> Result<Self, String> {
        let mut map = Grid::new(f.w, f.h);
        if f.w > 0 && f.h > 0 {
            let img = image::load_from_memory_with_format(&base64_decode(&f.png)?, image::ImageFormat::Png).map_err(|e| e.to_string())?.to_luma8();
            if (img.width() as usize, img.height() as usize) != (f.w, f.h) {
                return Err("mapa de cobertura con un tamaño distinto del indicado".into());
            }
            map.data = img.as_raw().iter().map(|v| *v as f32 / 255.0).collect();
        }
        Ok(Coverage { map, mm_per_px: f.mm_per_px })
    }
}

/// Lo que determina los trazos de una tinta: si no cambia, no se recalculan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraceKey {
    pub trace: TraceParams,
    pub pen_width_mm: f64,
    /// Tintas con las que comparte las líneas. No se guarda: al abrir el archivo, las
    /// tintas en relevo se recalculan.
    #[serde(skip)]
    pub relay: Option<Relay>,
}

/// Relevo entre tintas de una misma foto. Todas calculan las mismas líneas sobre el
/// color combinado y cada una se queda con los tramos que le tocan, así que una línea
/// pasa de un rotulador a otro sin huecos ni solapes.
#[derive(Clone, Debug, PartialEq)]
pub struct Relay {
    /// Mapas de todas las tintas del relevo, en el orden del documento.
    pub inks: Vec<Shared<Coverage>>,
    /// Posición de esta tinta en `inks`.
    pub index: usize,
    /// Trazo de las líneas compartidas (el de la primera tinta).
    pub look: TraceParams,
    /// Grosor común con el que se calculan las líneas compartidas, mm.
    pub pen_width_mm: f64,
    pub seed: usize,
}

/// Una tinta trazada a partir de su mapa de cobertura.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Traced {
    pub coverage: Shared<Coverage>,
    pub trace: TraceParams,
    /// Desfasa los umbrales para que las tintas de una misma foto se intercalen.
    pub seed: usize,
    /// Trazos ya calculados, con el centro del mapa en el origen.
    pub lines: Shared<Vec<Polyline>>,
    /// Ajustes con los que se calcularon `lines` (`None`: aún sin calcular).
    pub made: Option<TraceKey>,
}

impl Traced {
    /// Tinta sin trazos todavía; la app los calcula en segundo plano.
    pub fn new(coverage: Coverage, trace: TraceParams, seed: usize) -> Self {
        Traced { coverage: Shared::new(coverage), trace, seed, lines: Shared::new(vec![]), made: None }
    }

    pub fn key(&self, pen_width_mm: f64) -> TraceKey {
        TraceKey { trace: self.trace.clone(), pen_width_mm, relay: None }
    }

    /// ¿Corresponden los trazos guardados a los ajustes y al grosor actuales?
    pub fn is_current(&self, pen_width_mm: f64) -> bool {
        self.made.as_ref() == Some(&self.key(pen_width_mm))
    }

    /// Calcula los trazos (puede tardar: se llama fuera del hilo de la interfaz).
    pub fn generate(coverage: &Coverage, key: &TraceKey, seed: usize) -> Vec<Polyline> {
        let (w, h) = coverage.size_mm();
        let half = p(w / 2.0, h / 2.0);
        let lines = match &key.relay {
            Some(relay) => render_relay(relay, &key.trace, key.pen_width_mm),
            None => render(&coverage.map, coverage.mm_per_px, &key.trace, key.pen_width_mm, seed),
        };
        lines
            .into_iter()
            .map(|l| l.into_iter().map(|q| q - half).collect())
            .collect()
    }

    /// Guarda unos trazos calculados con `key`.
    pub fn set_lines(&mut self, key: TraceKey, lines: Vec<Polyline>) {
        self.lines = Shared::new(lines);
        self.made = Some(key);
    }
}

/// Trama de un mapa de cobertura, en mm con origen en la esquina superior izquierda del
/// mapa. `mm_per_px` es el tamaño en el papel de cada píxel; `seed` intercala tintas.
pub fn render(cov: &Grid<f32>, mm_per_px: f64, prm: &TraceParams, pen_width_mm: f64, seed: usize) -> Vec<Polyline> {
    let field = Field::new(cov, mm_per_px);
    let lines = base_lines(&field, prm, pen_width_mm, seed);
    if prm.weight <= 0.0 || prm.style == TraceStyle::Dots {
        return lines;
    }
    lines.into_par_iter().map(|l| thicken(&l, &field, prm.weight, pen_width_mm)).collect()
}

/// Líneas de un estilo con su vibración, todavía sin engordar. Los puntos ya salen
/// completos, con sus anillos.
fn base_lines(field: &Field, prm: &TraceParams, pen_width_mm: f64, seed: usize) -> Vec<Polyline> {
    let lines = match prm.style {
        TraceStyle::Hatch => hatch(field, prm, pen_width_mm, seed),
        TraceStyle::Flow => flow(field, prm, pen_width_mm, seed),
        TraceStyle::Dots => return dots(field, prm, pen_width_mm, seed),
        TraceStyle::Contour => contour(field, prm, pen_width_mm, seed),
        TraceStyle::Spiral => spiral(field, prm, pen_width_mm, seed),
        TraceStyle::Scribble => scribble(field, prm, pen_width_mm, seed),
    };
    let vibe = prm.style.uses_vibe() && prm.vibe != Vibe::None && prm.vibe_mm > 0.0;
    if !vibe {
        return lines;
    }
    lines.into_par_iter().enumerate().map(|(i, l)| vibrate(&l, field, prm, seed * 7919 + i)).collect()
}

/// Umbral de la secuencia R2 para el tramo `m` de la línea `k`.
fn relay_threshold(k: usize, m: i64) -> f64 {
    frac(0.5 + k as f64 * 0.754_877_666 + m as f64 * 0.569_840_291)
}

/// Qué tinta pinta en un punto: el umbral `t` cae en el reparto de su color.
fn relay_pick(c: &[f64], t: f64) -> Option<usize> {
    let total: f64 = c.iter().sum();
    if total < 1e-6 {
        return None;
    }
    let mut acc = 0.0;
    for (i, v) in c.iter().enumerate() {
        acc += v / total;
        if t < acc {
            return Some(i);
        }
    }
    Some(c.len() - 1)
}

/// Trazos de una tinta en relevo: las líneas compartidas, calculadas sobre el color
/// combinado de todas, y de ellas los tramos que le tocan a esta tinta. `own` son los
/// ajustes propios (solo cuenta su grosor) y `pen_width_mm` el de su rotulador.
fn render_relay(relay: &Relay, own: &TraceParams, pen_width_mm: f64) -> Vec<Polyline> {
    let me = relay.index;
    let Some(first) = relay.inks.get(me) else { return vec![] };
    let (w, h, k) = (first.map.w, first.map.h, first.mm_per_px);
    if relay.inks.iter().any(|c| (c.map.w, c.map.h) != (w, h)) {
        return vec![];
    }
    // Color combinado: lo que queda sin cubrir es el producto de lo que deja cada tinta.
    let mut combined = Grid::new(w, h);
    for (i, v) in combined.data.iter_mut().enumerate() {
        let blank: f32 = relay.inks.iter().map(|c| 1.0 - c.map.data[i].clamp(0.0, 1.0)).product();
        *v = 1.0 - blank;
    }
    let field = Field::new(&combined, k);
    let fields: Vec<Field> = relay.inks.iter().map(|c| Field::new(&c.map, k)).collect();
    let shares = |q: P| -> Vec<f64> { fields.iter().map(|f| f.at(q).max(0.0)).collect() };
    let look = &relay.look;
    let common = relay.pen_width_mm;
    if look.style == TraceStyle::Dots {
        // Cada punto entero para una tinta, según el color de su centro.
        return dots(&field, look, common, relay.seed)
            .into_iter()
            .enumerate()
            .filter(|(i, d)| {
                let c = d.iter().fold(p(0.0, 0.0), |a, q| a + *q) * (1.0 / d.len().max(1) as f64);
                relay_pick(&shares(c), relay_threshold(*i, 0)) == Some(me)
            })
            .map(|(_, d)| d)
            .collect();
    }
    let lines = base_lines(&field, &TraceParams { weight: 0.0, ..look.clone() }, common, relay.seed);
    let step = line_step(&field, look.spacing_mm.max(0.2));
    let min_len = (common * 1.2).max(0.4);
    let cell = look.dash_mm.max(0.5);
    let own_field = &fields[me];
    lines
        .par_iter()
        .enumerate()
        .flat_map_iter(|(i, l)| {
            relay_pieces(l, i, &fields, step, cell, min_len, me).into_iter().map(|piece| {
                if own.weight > 0.0 { thicken(&piece, own_field, own.weight, pen_width_mm) } else { piece }
            })
        })
        .collect()
}

/// Reparte la línea `k` entre las tintas y devuelve los tramos de `me`. La línea se
/// divide en celdas de `cell` mm, cada una con su umbral; en cada punto pinta la tinta
/// en cuyo reparto de color cae el umbral. Los tramos más cortos que `min_len` se unen
/// al vecino, y dos tramos seguidos se tocan en el mismo punto.
fn relay_pieces(l: &[P], k: usize, fields: &[Field], step: f64, cell: f64, min_len: f64, me: usize) -> Vec<Polyline> {
    if l.len() < 2 {
        return vec![];
    }
    let pts = densify(l, step);
    let shift = frac(k as f64 * 0.381_966) * cell;
    let mut owners: Vec<usize> = Vec::with_capacity(pts.len());
    let (mut cur, mut last_m) = (None::<usize>, i64::MIN);
    for &(q, arc) in &pts {
        let m = ((arc + shift) / cell).floor() as i64;
        let t = relay_threshold(k, m);
        let c: Vec<f64> = fields.iter().map(|f| f.at(q).max(0.0)).collect();
        let new = relay_pick(&c, t);
        // Dentro de una celda, histéresis para que el borde de un color no parta la línea
        // en trocitos; al cambiar de celda manda el umbral nuevo.
        let keep = match (cur, new) {
            (Some(o), Some(n)) if o != n && m == last_m && c[o] > 0.03 => {
                let total: f64 = c.iter().sum();
                let lo = c[..o].iter().sum::<f64>() / total;
                let hi = lo + c[o] / total;
                t >= lo - 0.05 && t < hi + 0.05
            }
            (Some(_), None) => true,
            _ => false,
        };
        if !keep {
            cur = new.or(cur);
        }
        last_m = m;
        owners.push(cur.unwrap_or(0));
    }
    // Tramos seguidos de la misma tinta: (tinta, primer punto, último punto).
    let mut runs: Vec<(usize, usize, usize)> = Vec::new();
    for (i, &o) in owners.iter().enumerate() {
        match runs.last_mut() {
            Some(r) if r.0 == o => r.2 = i,
            _ => runs.push((o, i, i)),
        }
    }
    // Longitud de un tramo, hasta el punto medio con sus vecinos.
    let run_len = |r: &(usize, usize, usize)| {
        let a = if r.1 > 0 { (pts[r.1 - 1].1 + pts[r.1].1) / 2.0 } else { 0.0 };
        let b = if r.2 + 1 < pts.len() { (pts[r.2].1 + pts[r.2 + 1].1) / 2.0 } else { pts[r.2].1 };
        b - a
    };
    while runs.len() > 1 {
        let Some((i, _)) = runs.iter().enumerate().map(|(i, r)| (i, run_len(r))).filter(|(_, len)| *len < min_len).min_by(|a, b| a.1.total_cmp(&b.1)) else { break };
        // Se une al vecino más largo.
        let j = match (i.checked_sub(1), (i + 1 < runs.len()).then_some(i + 1)) {
            (Some(a), Some(b)) => if run_len(&runs[a]) >= run_len(&runs[b]) { a } else { b },
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => break,
        };
        let (lo, hi) = (i.min(j), i.max(j));
        runs[lo] = (runs[j].0, runs[lo].1, runs[hi].2);
        runs.remove(hi);
        // Si quedan dos tramos seguidos de la misma tinta, se juntan.
        let mut merged: Vec<(usize, usize, usize)> = Vec::with_capacity(runs.len());
        for r in runs.drain(..) {
            match merged.last_mut() {
                Some(m) if m.0 == r.0 => m.2 = r.2,
                _ => merged.push(r),
            }
        }
        runs = merged;
    }
    runs.iter()
        .filter(|r| r.0 == me)
        .filter_map(|r| {
            let mut piece = Vec::with_capacity(r.2 - r.1 + 3);
            if r.1 > 0 {
                piece.push(pts[r.1 - 1].0.lerp(pts[r.1].0, 0.5));
            }
            piece.extend(pts[r.1..=r.2].iter().map(|(q, _)| *q));
            if r.2 + 1 < pts.len() {
                piece.push(pts[r.2].0.lerp(pts[r.2 + 1].0, 0.5));
            }
            (piece.len() >= 2).then(|| crate::geom::simplify(&piece, 0.02))
        })
        .collect()
}

/// Mapa de cobertura leído en mm; fuera del mapa no hay color.
struct Field<'a> {
    cov: &'a Grid<f32>,
    k: f64,
    w: f64,
    h: f64,
}

impl<'a> Field<'a> {
    fn new(cov: &'a Grid<f32>, k: f64) -> Self {
        Field { cov, k, w: cov.w as f64 * k, h: cov.h as f64 * k }
    }
    fn inside(&self, q: P) -> bool {
        q.x >= 0.0 && q.y >= 0.0 && q.x <= self.w && q.y <= self.h
    }
    fn at(&self, q: P) -> f64 {
        if self.inside(q) { sample(self.cov, q.x / self.k, q.y / self.k) } else { 0.0 }
    }
}

fn sample(g: &Grid<f32>, x: f64, y: f64) -> f64 {
    if g.w == 0 || g.h == 0 {
        return 0.0;
    }
    let x = (x - 0.5).clamp(0.0, (g.w - 1) as f64);
    let y = (y - 0.5).clamp(0.0, (g.h - 1) as f64);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(g.w - 1), (y0 + 1).min(g.h - 1));
    let (fx, fy) = (x - x0 as f64, y - y0 as f64);
    let a = g.get(x0, y0) as f64 * (1.0 - fx) + g.get(x1, y0) as f64 * fx;
    let b = g.get(x0, y1) as f64 * (1.0 - fx) + g.get(x1, y1) as f64 * fx;
    a * (1.0 - fy) + b * fy
}

fn frac(v: f64) -> f64 {
    v - v.floor()
}

/// Paso de muestreo a lo largo de las líneas.
fn line_step(field: &Field, spacing: f64) -> f64 {
    (spacing * 0.4).min(field.k).max(0.08)
}

/// Recorre un camino denso y deja los tramos con color suficiente. Cada tramo de
/// `dash_mm` (más o menos, según `dash_vary`) tiene su umbral de la secuencia R2: en
/// tonos claros salen trazos cortos bien repartidos y en los oscuros se juntan en
/// líneas largas. `line` identifica el camino para repartir los umbrales.
fn dashes(path: &[P], field: &Field, prm: &TraceParams, line: i64, seed: usize, min_len: f64, keep_points: bool) -> Vec<Polyline> {
    let dash = prm.dash_mm.max(0.3);
    let vary = prm.dash_vary.clamp(0.0, 1.0);
    let lf = line as f64;
    let cell = |m: i64| dash * (1.0 + 0.85 * vary * (2.0 * frac(m as f64 * 0.618_034 + lf * 0.414_214 + seed as f64 * 0.137) - 1.0));
    let threshold = |m: i64| 0.03 + 0.94 * frac(lf * 0.754_877_666 + m as f64 * 0.569_840_291 + seed as f64 * 0.31);
    let mut out = Vec::new();
    let mut flush = |cur: &mut Vec<P>| {
        let seg = std::mem::take(cur);
        if seg.len() >= 2 && crate::geom::length(&seg) >= min_len {
            out.push(if keep_points { crate::geom::simplify(&seg, 0.02) } else { vec![seg[0], *seg.last().unwrap()] });
        }
    };
    let mut cur: Vec<P> = Vec::new();
    let (mut on, mut m, mut arc) = (false, 0i64, 0.0);
    // Con variación, cada línea empieza a mitad de un tramo para que no se alineen.
    let mut end = cell(0) * (1.0 - 0.9 * vary * frac(lf * 0.381_966));
    for (i, &q) in path.iter().enumerate() {
        if i > 0 {
            arc += q.dist(path[i - 1]);
        }
        while arc > end {
            m += 1;
            end += cell(m);
        }
        let d = field.at(q);
        let t = threshold(m);
        // Histéresis para no fragmentar las líneas en el borde de un tono.
        on = if on { d > t - 0.03 } else { d > t + 0.03 };
        if on {
            cur.push(q);
        } else if !cur.is_empty() {
            flush(&mut cur);
        }
    }
    flush(&mut cur);
    out
}

fn hatch(field: &Field, prm: &TraceParams, pen_width_mm: f64, seed: usize) -> Vec<Polyline> {
    let s = prm.spacing_mm.max(0.2);
    let a = prm.angle_deg.to_radians();
    let (u, n) = (p(a.cos(), a.sin()), p(-a.sin(), a.cos()));
    let corners = [p(0.0, 0.0), p(field.w, 0.0), p(0.0, field.h), p(field.w, field.h)];
    let proj = |d: P| corners.iter().map(|c| c.x * d.x + c.y * d.y).fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
    let (n0, n1) = proj(n);
    let (u0, u1) = proj(u);
    let step = line_step(field, s);
    let min_len = (pen_width_mm * 1.2).max(0.4);
    let first = (n0 / s).floor() as i64;
    let last = (n1 / s).ceil() as i64;
    let lines: Vec<Vec<Polyline>> = (first..=last)
        .into_par_iter()
        .map(|j| {
            let o = (j as f64 + prm.phase) * s;
            let count = ((u1 + step - u0) / step).ceil() as usize + 1;
            let path: Vec<P> = (0..count).map(|i| n * o + u * (u0 + i as f64 * step)).collect();
            let mut out = dashes(&path, field, prm, j, seed, min_len, false);
            if j % 2 != 0 {
                out.reverse();
                for l in &mut out {
                    l.reverse();
                }
            }
            out
        })
        .collect();
    lines.into_iter().flatten().collect()
}

/// Desenfoque de caja separable, repetido para parecerse a uno gaussiano.
fn blur(g: &Grid<f32>, r: usize) -> Grid<f32> {
    let mut cur = g.clone();
    if r == 0 || g.w == 0 || g.h == 0 {
        return cur;
    }
    let pass = |src: &Grid<f32>, horizontal: bool| -> Grid<f32> {
        let (len, lines) = if horizontal { (src.w, src.h) } else { (src.h, src.w) };
        let idx = |line: usize, i: usize| if horizontal { line * src.w + i } else { i * src.w + line };
        let mut dst = Grid::new(src.w, src.h);
        let norm = 1.0 / (2 * r + 1) as f32;
        for line in 0..lines {
            let at = |i: isize| src.data[idx(line, i.clamp(0, len as isize - 1) as usize)];
            let mut sum: f32 = (-(r as isize)..=r as isize).map(at).sum();
            for i in 0..len {
                dst.data[idx(line, i)] = sum * norm;
                sum += at(i as isize + r as isize + 1) - at(i as isize - r as isize);
            }
        }
        dst
    };
    for _ in 0..2 {
        cur = pass(&pass(&cur, true), false);
    }
    cur
}

/// Campo de direcciones de Flujo, guardado como ángulo doble (cos 2θ, sin 2θ) para
/// poder interpolarlo sin que la dirección «salte» de sentido. Junto a los bordes del
/// color sigue el contorno; en las zonas lisas se acerca al ángulo de la tinta.
struct Directions {
    c2: Grid<f32>,
    s2: Grid<f32>,
    base: P,
}

impl Directions {
    fn new(field: &Field, prm: &TraceParams) -> Self {
        let g = blur(field.cov, 1);
        let (w, h) = (g.w, g.h);
        let (mut jxx, mut jxy, mut jyy) = (Grid::new(w, h), Grid::new(w, h), Grid::new(w, h));
        for y in 0..h {
            for x in 0..w {
                let gx = (g.at(x as isize + 1, y as isize) - g.at(x as isize - 1, y as isize)) * 0.5;
                let gy = (g.at(x as isize, y as isize + 1) - g.at(x as isize, y as isize - 1)) * 0.5;
                jxx.set(x, y, gx * gx);
                jxy.set(x, y, gx * gy);
                jyy.set(x, y, gy * gy);
            }
        }
        // Radio del promedio: lo bastante amplio para que la forma de los bordes entre
        // en las zonas lisas cercanas.
        let r = ((prm.spacing_mm.max(0.2) * 3.0 / field.k).round() as usize).clamp(2, 30);
        let (jxx, jxy, jyy) = (blur(&jxx, r), blur(&jxy, r), blur(&jyy, r));
        let a = prm.angle_deg.to_radians();
        let base = p((2.0 * a).cos(), (2.0 * a).sin());
        let mags: Vec<f32> = (0..w * h).map(|i| (jxx.data[i] - jyy.data[i]).hypot(2.0 * jxy.data[i])).collect();
        let (sum, count) = mags.iter().filter(|m| **m > 1e-6).fold((0.0f64, 0usize), |(s, c), m| (s + *m as f64, c + 1));
        let eps = if count > 0 { (sum / count as f64 * 0.25) as f32 } else { 1.0 };
        let (mut c2, mut s2) = (Grid::new(w, h), Grid::new(w, h));
        for (i, &m) in mags.iter().enumerate() {
            let blend = m / (m + eps);
            // El gradiente apunta a través del borde; la isolínea va a 90°: ángulo doble + 180°.
            let (gc, gs) = if m > 1e-9 { (-(jxx.data[i] - jyy.data[i]) / m, -2.0 * jxy.data[i] / m) } else { (0.0, 0.0) };
            c2.data[i] = gc * blend + base.x as f32 * (1.0 - blend);
            s2.data[i] = gs * blend + base.y as f32 * (1.0 - blend);
        }
        Directions { c2, s2, base }
    }

    /// Dirección (sin sentido) en un punto en mm.
    fn at(&self, field: &Field, q: P) -> P {
        let (c, s) = (sample(&self.c2, q.x / field.k, q.y / field.k), sample(&self.s2, q.x / field.k, q.y / field.k));
        let (c, s) = if c.hypot(s) < 1e-6 { (self.base.x, self.base.y) } else { (c, s) };
        let t = 0.5 * s.atan2(c);
        p(t.cos(), t.sin())
    }
}

/// Puntos de las líneas ya trazadas, por celdas, para mantener la separación.
struct Occupancy {
    cell: f64,
    cols: usize,
    rows: usize,
    cells: Vec<Vec<(P, u32, i32)>>,
    alive: Vec<bool>,
}

impl Occupancy {
    fn new(field: &Field, cell: f64) -> Self {
        let cols = (field.w / cell).ceil() as usize + 1;
        let rows = (field.h / cell).ceil() as usize + 1;
        Occupancy { cell, cols, rows, cells: vec![Vec::new(); cols * rows], alive: Vec::new() }
    }
    fn slot(&self, q: P) -> (isize, isize) {
        ((q.x / self.cell).floor() as isize, (q.y / self.cell).floor() as isize)
    }
    fn add(&mut self, q: P, line: u32, index: i32) {
        let (cx, cy) = self.slot(q);
        if cx >= 0 && cy >= 0 && (cx as usize) < self.cols && (cy as usize) < self.rows {
            self.cells[cy as usize * self.cols + cx as usize].push((q, line, index));
        }
    }
    /// ¿Hay algún punto a menos de `r` (≤ celda)? Los puntos cercanos de la misma
    /// línea (`skip` posiciones alrededor de `index`) no cuentan.
    fn near(&self, q: P, r: f64, line: u32, index: i32, skip: i32) -> bool {
        let (cx, cy) = self.slot(q);
        for y in cy - 1..=cy + 1 {
            for x in cx - 1..=cx + 1 {
                if x < 0 || y < 0 || x as usize >= self.cols || y as usize >= self.rows {
                    continue;
                }
                for &(o, l, i) in &self.cells[y as usize * self.cols + x as usize] {
                    if !self.alive[l as usize] || (l == line && (i - index).abs() <= skip) {
                        continue;
                    }
                    if o.dist(q) < r {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// Flujo: líneas de corriente separadas por igual (Jobard y Lefer) sobre el campo de
/// direcciones. Una línea se para cuando sale de su color o se acerca demasiado a otra;
/// después se trocea con los mismos umbrales que el rayado para dar el tono.
fn flow(field: &Field, prm: &TraceParams, pen_width_mm: f64, seed: usize) -> Vec<Polyline> {
    const STOP: f64 = 0.04;
    let s = prm.spacing_mm.max(0.25);
    let step = line_step(field, s);
    let test = s * 0.55;
    let skip = (test * 2.5 / step).ceil() as i32 + 2;
    let max_steps = ((field.w + field.h) * 3.0 / step) as usize;
    let min_len = (pen_width_mm * 1.2).max(0.4);
    let dirs = Directions::new(field, prm);
    let mut occ = Occupancy::new(field, s);
    let mut paths: Vec<Vec<P>> = Vec::new();

    let try_seed = |occ: &mut Occupancy, start: P, paths: &mut Vec<Vec<P>>| -> bool {
        let id = occ.alive.len() as u32;
        if field.at(start) < STOP || occ.near(start, s * 0.8, u32::MAX, 0, 0) {
            return false;
        }
        occ.alive.push(true);
        occ.add(start, id, 0);
        let mut halves: [Vec<P>; 2] = [Vec::new(), Vec::new()];
        for (h, sign) in [(0usize, 1.0), (1usize, -1.0)] {
            let mut q = start;
            let mut prev = dirs.at(field, start) * sign;
            for n in 1..=max_steps {
                let align = |d: P, prev: P| if d.dot(prev) < 0.0 { d * -1.0 } else { d };
                let d1 = align(dirs.at(field, q), prev);
                let d2 = align(dirs.at(field, q + d1 * (step * 0.5)), d1);
                let next = q + d2 * step;
                let index = if h == 0 { n as i32 } else { -(n as i32) };
                if field.at(next) < STOP || occ.near(next, test, id, index, skip) {
                    break;
                }
                occ.add(next, id, index);
                halves[h].push(next);
                prev = d2;
                q = next;
            }
        }
        let [fwd, mut back] = halves;
        back.reverse();
        back.push(start);
        back.extend(fwd);
        if crate::geom::length(&back) < (s * 1.5).max(min_len) {
            occ.alive[id as usize] = false;
            return false;
        }
        paths.push(back);
        true
    };

    // Semillas: a un lado y otro de cada línea nueva; cuando se agotan, se busca color
    // sin cubrir recorriendo el mapa.
    let gap = (s / step).ceil().max(1.0) as usize;
    let jitter = p(frac(seed as f64 * 0.618_034 + prm.phase), frac(seed as f64 * 0.414_214 + prm.phase)) * s;
    let (cols, rows) = ((field.w / s).ceil() as usize + 1, (field.h / s).ceil() as usize + 1);
    let mut scan = 0usize;
    let mut queue = 0usize;
    loop {
        if queue < paths.len() {
            let path = paths[queue].clone();
            queue += 1;
            for i in (0..path.len()).step_by(gap) {
                let a = path[i.saturating_sub(1)];
                let b = path[(i + 1).min(path.len() - 1)];
                let d = b - a;
                let len = d.len();
                if len < 1e-9 {
                    continue;
                }
                let nrm = p(-d.y / len, d.x / len);
                for side in [1.0, -1.0] {
                    try_seed(&mut occ, path[i] + nrm * (s * side), &mut paths);
                }
            }
            continue;
        }
        let mut found = false;
        while scan < cols * rows {
            let q = p((scan % cols) as f64 * s, (scan / cols) as f64 * s) + jitter;
            scan += 1;
            if try_seed(&mut occ, q, &mut paths) {
                found = true;
                break;
            }
        }
        if !found {
            break;
        }
    }

    paths
        .par_iter()
        .enumerate()
        .flat_map_iter(|(i, path)| dashes(path, field, prm, i as i64, seed, min_len, true))
        .collect()
}

/// Contornos: curvas a igual distancia del borde de cada mancha (como curvas de nivel),
/// troceadas con los umbrales del rayado para dar el tono.
fn contour(field: &Field, prm: &TraceParams, pen_width_mm: f64, seed: usize) -> Vec<Polyline> {
    let s = prm.spacing_mm.max(0.25);
    let min_len = (pen_width_mm * 1.2).max(0.4);
    let cov = field.cov;
    // Un píxel de margen para que las manchas que tocan el borde del mapa se cierren.
    let mut mask = Grid::<bool>::new(cov.w + 2, cov.h + 2);
    for y in 0..cov.h {
        for x in 0..cov.w {
            mask.set(x + 1, y + 1, cov.get(x, y) > 0.04);
        }
    }
    let dist = crate::raster::distance_inside(&mask);
    let deepest = dist.data.iter().cloned().fold(0.0f32, f32::max) as f64 * field.k;
    let levels = ((deepest / s) - prm.phase.rem_euclid(1.0)).max(0.0).ceil() as usize + 1;
    let k = field.k;
    let rings: Vec<Vec<Polyline>> = (0..levels)
        .into_par_iter()
        .map(|i| {
            let level = ((i as f64 + 0.5 + prm.phase.rem_euclid(1.0)) * s / k) as f32;
            let mut out = Vec::new();
            for (r, ring) in crate::raster::contours(&dist, level).into_iter().enumerate() {
                if ring.len() < 3 {
                    continue;
                }
                let closed = ring.first() == ring.last();
                let pts: Vec<P> = ring.iter().map(|q| p((q.x - 1.0) * k, (q.y - 1.0) * k)).collect();
                let smooth = if closed { crate::geom::chaikin(&pts[..pts.len() - 1], 2, true) } else { crate::geom::chaikin(&pts, 2, false) };
                out.extend(dashes(&smooth, field, prm, (i * 7919 + r) as i64, seed, min_len, true));
            }
            out
        })
        .collect();
    rings.into_iter().flatten().collect()
}

/// Espiral de Arquímedes centrada en el mapa (desplazada por tinta para que aparezca
/// moiré). Donde hay color se ondula hacia fuera y hacia dentro, más cuanto más color,
/// y se trocea con los umbrales del rayado.
fn spiral(field: &Field, prm: &TraceParams, pen_width_mm: f64, seed: usize) -> Vec<Polyline> {
    let s = prm.spacing_mm.max(0.3);
    let min_len = (pen_width_mm * 1.2).max(0.4);
    let off = if seed == 0 { p(0.0, 0.0) } else { p((seed as f64 * 2.4).cos(), (seed as f64 * 2.4).sin()) * (s * 2.5) };
    let c = p(field.w / 2.0, field.h / 2.0) + off;
    let rmax = [p(0.0, 0.0), p(field.w, 0.0), p(0.0, field.h), p(field.w, field.h)].iter().map(|q| q.dist(c)).fold(0.0, f64::max) + s;
    let wave = (pen_width_mm * 2.2).max(0.5);
    let amp = s * 0.45;
    let step = (wave / 6.0).min(line_step(field, s));
    let mut path = Vec::new();
    let (mut theta, mut arc) = (0.0f64, 0.0f64);
    let start = prm.phase.rem_euclid(1.0) * s;
    loop {
        let r = start + s * theta / std::f64::consts::TAU;
        if r > rmax {
            break;
        }
        let dir = p(theta.cos(), theta.sin());
        let d = field.at(c + dir * r);
        let wobble = (arc / wave * std::f64::consts::TAU).sin() * amp * d;
        path.push(c + dir * (r + wobble).max(0.0));
        theta += step / r.max(s);
        arc += step;
    }
    dashes(&path, field, prm, seed as i64, seed, min_len, true)
}

/// Garabato: puntos repartidos según el tono, unidos por el vecino más cercano y
/// suavizados. En los oscuros hay más puntos y el trazo se enreda; si el siguiente
/// punto queda lejos se levanta el rotulador.
fn scribble(field: &Field, prm: &TraceParams, pen_width_mm: f64, seed: usize) -> Vec<Polyline> {
    let s = prm.spacing_mm.max(0.3);
    let cell = s;
    let row_h = cell * 0.866;
    let shift = p(frac(seed as f64 * 0.618_034 + prm.phase), frac(seed as f64 * 0.414_214 + prm.phase)) * cell;
    let (cols, rows) = ((field.w / cell).ceil() as usize + 1, (field.h / row_h).ceil() as usize + 1);
    let mut pts = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            let i = row * cols + col;
            let t = frac(col as f64 * 0.754_877_666 + row as f64 * 0.569_840_291 + seed as f64 * 0.31);
            let j = p(frac(t * 7.31 + i as f64 * 0.137) - 0.5, frac(t * 3.17 + i as f64 * 0.291) - 0.5) * (cell * 0.9);
            let q = p(col as f64 * cell + if row % 2 == 1 { cell / 2.0 } else { 0.0 }, row as f64 * row_h) + shift + j;
            if field.at(q) > 0.02 + t * 0.96 {
                pts.push(q);
            }
        }
    }
    if pts.is_empty() {
        return vec![];
    }
    // Vecino más cercano con celdas de búsqueda.
    let (gc, gr) = ((field.w / cell).ceil() as usize + 2, (field.h / cell).ceil() as usize + 2);
    let slot = |q: P| (((q.x / cell).floor().max(0.0) as usize).min(gc - 1), ((q.y / cell).floor().max(0.0) as usize).min(gr - 1));
    let mut cells: Vec<Vec<usize>> = vec![Vec::new(); gc * gr];
    for (i, q) in pts.iter().enumerate() {
        let (x, y) = slot(*q);
        cells[y * gc + x].push(i);
    }
    let reach = 2isize;
    let mut used = vec![false; pts.len()];
    let mut lines = Vec::new();
    let mut next_start = 0usize;
    loop {
        while next_start < pts.len() && used[next_start] {
            next_start += 1;
        }
        if next_start >= pts.len() {
            break;
        }
        let mut cur = next_start;
        let mut line = vec![pts[cur]];
        loop {
            used[cur] = true;
            let (cx, cy) = slot(pts[cur]);
            cells[cy * gc + cx].retain(|i| *i != cur);
            let mut best: Option<(f64, usize)> = None;
            for y in (cy as isize - reach).max(0)..=(cy as isize + reach).min(gr as isize - 1) {
                for x in (cx as isize - reach).max(0)..=(cx as isize + reach).min(gc as isize - 1) {
                    for &i in &cells[y as usize * gc + x as usize] {
                        let d = pts[i].dist(pts[cur]);
                        if best.is_none_or(|(b, _)| d < b) {
                            best = Some((d, i));
                        }
                    }
                }
            }
            match best {
                Some((d, i)) if d <= cell * reach as f64 => {
                    line.push(pts[i]);
                    cur = i;
                }
                _ => break,
            }
        }
        if line.len() >= 2 {
            lines.push(line);
        }
    }
    let min_len = (pen_width_mm * 1.2).max(0.4);
    lines
        .into_par_iter()
        .flat_map_iter(|l| inside_parts(&crate::geom::catmull_rom(&l, 5, false), field, min_len))
        .collect()
}

/// Parte una línea donde sale del color.
fn inside_parts(l: &[P], field: &Field, min_len: f64) -> Vec<Polyline> {
    let mut out = Vec::new();
    let mut cur: Vec<P> = Vec::new();
    for q in l {
        if field.at(*q) >= 0.03 {
            cur.push(*q);
        } else if !cur.is_empty() {
            let part = std::mem::take(&mut cur);
            if crate::geom::length(&part) >= min_len {
                out.push(part);
            }
        }
    }
    if cur.len() >= 2 && crate::geom::length(&cur) >= min_len {
        out.push(cur);
    }
    out
}

/// Normal unitaria de una polilínea en el punto `i`.
fn normal_at(l: &[P], i: usize) -> P {
    let d = l[(i + 1).min(l.len() - 1)] - l[i.saturating_sub(1)];
    let len = d.len();
    if len < 1e-12 { p(0.0, 0.0) } else { p(-d.y / len, d.x / len) }
}

/// La polilínea con todos sus vértices y puntos intermedios para que ningún tramo mida
/// más de `ds` mm, con la distancia de cada punto desde el principio.
fn densify(l: &[P], ds: f64) -> Vec<(P, f64)> {
    let mut out = vec![(l[0], 0.0)];
    let mut arc = 0.0;
    for w in l.windows(2) {
        let len = w[0].dist(w[1]);
        let n = (len / ds).ceil().max(1.0) as usize;
        for i in 1..=n {
            let t = i as f64 / n as f64;
            out.push((w[0].lerp(w[1], t), arc + len * t));
        }
        arc += len;
    }
    out
}

/// Puntos de una polilínea cada `ds` mm, con su distancia desde el principio.
fn resample(l: &[P], ds: f64) -> Vec<(P, f64)> {
    let mut out = vec![(l[0], 0.0)];
    let (mut arc, mut next) = (0.0, ds);
    for w in l.windows(2) {
        let len = w[0].dist(w[1]);
        while next <= arc + len && len > 0.0 {
            out.push((w[0].lerp(w[1], (next - arc) / len), next));
            next += ds;
        }
        arc += len;
    }
    if out.last().map(|(_, a)| arc - a > ds * 0.2).unwrap_or(true) {
        out.push((*l.last().unwrap(), arc));
    }
    out
}

/// Añade vibración a una línea. La amplitud se reduce si el punto desplazado se sale
/// del color, para que la línea no invada a la tinta vecina.
fn vibrate(l: &[P], field: &Field, prm: &TraceParams, key: usize) -> Polyline {
    if l.len() < 2 {
        return l.to_vec();
    }
    let wave = prm.wave_mm.max(0.2);
    let ds = if prm.vibe == Vibe::Zigzag { wave / 4.0 } else { wave / 10.0 };
    let pts = resample(l, ds);
    let base: Vec<P> = pts.iter().map(|(q, _)| *q).collect();
    let rnd = |n: i64| 2.0 * frac(((key as f64 * 12.9898 + n as f64 * 78.233).sin()) * 43_758.545) - 1.0;
    let phase = frac(key as f64 * 0.618_034) * std::f64::consts::TAU;
    pts.iter()
        .enumerate()
        .map(|(i, &(q, arc))| {
            let t = arc / wave;
            let off = match prm.vibe {
                Vibe::None => 0.0,
                Vibe::Wave => (t * std::f64::consts::TAU + phase).sin(),
                // Muestreado cada cuarto de onda: 0, 1, 0, −1… dientes rectos.
                Vibe::Zigzag => [0.0, 1.0, 0.0, -1.0][((t * 4.0).round() as i64).rem_euclid(4) as usize],
                Vibe::Tremor => {
                    let (n, f) = ((t * 2.0).floor() as i64, frac(t * 2.0));
                    let e = (1.0 - (f * std::f64::consts::PI).cos()) * 0.5;
                    rnd(n) * (1.0 - e) + rnd(n + 1) * e
                }
            };
            let amp = prm.vibe_mm * if prm.vibe_by_tone { field.at(q).clamp(0.0, 1.0) } else { 1.0 };
            let nrm = normal_at(&base, i);
            let mut moved = q + nrm * (off * amp);
            for shrink in [0.5, 0.0] {
                if field.at(moved) >= 0.03 {
                    break;
                }
                moved = q + nrm * (off * amp * shrink);
            }
            moved
        })
        .collect()
}

/// Repasa la línea de ida y vuelta, desplazada un poco cada vez, donde hay más color:
/// el trazo se ve más grueso en los oscuros.
fn thicken(l: &[P], field: &Field, weight: f64, pen_width_mm: f64) -> Polyline {
    if l.len() < 2 {
        return l.to_vec();
    }
    let tone = l.iter().map(|q| field.at(*q)).sum::<f64>() / l.len() as f64;
    let passes = 1 + (weight.clamp(0.0, 1.0) * 3.0 * tone).round() as usize;
    if passes <= 1 {
        return l.to_vec();
    }
    let gap = pen_width_mm.max(0.1) * 0.85;
    let normals: Vec<P> = (0..l.len()).map(|i| normal_at(l, i)).collect();
    let mut out = Vec::with_capacity(l.len() * passes);
    for k in 0..passes {
        let off = (k as f64 - (passes - 1) as f64 / 2.0) * gap;
        let pass = l.iter().zip(&normals).map(|(q, n)| *q + *n * off);
        if k % 2 == 0 { out.extend(pass) } else { out.extend(pass.rev()) }
    }
    out
}

fn dots(field: &Field, prm: &TraceParams, pen_width_mm: f64, index: usize) -> Vec<Polyline> {
    let (wm, hm) = (field.w, field.h);
    let s = prm.spacing_mm.max(0.3);
    let row_h = s * 0.866;
    let r = (pen_width_mm * 0.25).max(0.1);
    let gap = pen_width_mm.max(0.1) * 0.85;
    let weight = prm.weight.clamp(0.0, 1.0);
    let rows = (hm / row_h).ceil() as usize + 1;
    // Cada rotulador desplaza su rejilla para que los puntos de colores se intercalen.
    let shift = p(frac(index as f64 * 0.618_034 + prm.phase) * s, frac(index as f64 * 0.414_214 + prm.phase) * row_h);
    let out: Vec<Vec<Polyline>> = (0..rows)
        .into_par_iter()
        .map(|row| {
            let y = row as f64 * row_h + shift.y;
            let off = if row % 2 == 0 { 0.0 } else { s / 2.0 };
            let mut v = Vec::new();
            let mut i = 0usize;
            let mut x = off + shift.x;
            while x < wm {
                // Umbral R2 (secuencia de baja discrepancia en 2D).
                let t = frac(i as f64 * 0.754_877_666 + row as f64 * 0.569_840_291 + index as f64 * 0.31);
                let jx = (frac(t * 7.31) - 0.5) * s * 0.18;
                let jy = (frac(t * 3.17) - 0.5) * s * 0.18;
                let q = p(x + jx, y + jy);
                let d = field.at(q);
                if field.inside(q) && d > 0.02 + t * 0.96 {
                    // Con grosor, los puntos de los oscuros crecen en anillos.
                    let rings = 1 + (weight * 3.0 * d).round() as usize;
                    let mut dot = Vec::new();
                    for ring in 0..rings {
                        let rr = r + ring as f64 * gap;
                        // Un anillo que pisaría fuera del color no se dibuja.
                        if ring > 0 && [p(rr, 0.0), p(-rr, 0.0), p(0.0, rr), p(0.0, -rr)].iter().any(|o| field.at(q + *o) < 0.03) {
                            break;
                        }
                        let n = ((std::f64::consts::TAU * rr / 0.2).ceil() as usize).max(6);
                        dot.extend((0..=n).map(|a| q + p(rr, 0.0).rotate(a as f64 / n as f64 * std::f64::consts::TAU)));
                    }
                    v.push(dot);
                }
                x += s;
                i += 1;
            }
            if row % 2 == 1 {
                v.reverse();
            }
            v
        })
        .collect();
    out.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn half_map() -> Coverage {
        let mut map = Grid::<f32>::new(40, 20);
        for y in 0..20 {
            for x in 0..20 {
                map.set(x, y, 0.9);
            }
        }
        Coverage { map, mm_per_px: 0.5 }
    }

    #[test]
    fn coverage_survives_saving_with_256_levels() {
        let mut cov = half_map();
        cov.map.set(30, 5, 0.25);
        let json = serde_json::to_string(&cov).unwrap();
        let back: Coverage = serde_json::from_str(&json).unwrap();
        assert_eq!((back.map.w, back.map.h, back.mm_per_px), (40, 20, 0.5));
        for (a, b) in cov.map.data.iter().zip(&back.map.data) {
            assert!((a - b).abs() <= 0.5 / 255.0 + 1e-6);
        }
    }

    #[test]
    fn traced_lines_are_centered_and_stay_inside_their_color() {
        let mut ink = Traced::new(half_map(), TraceParams { angle_deg: 0.0, ..Default::default() }, 0);
        assert!(!ink.is_current(0.5));
        let key = ink.key(0.5);
        let lines = Traced::generate(&ink.coverage, &key, ink.seed);
        ink.set_lines(key, lines);
        assert!(ink.is_current(0.5) && !ink.is_current(0.8));
        assert!(!ink.lines.is_empty());
        // El mapa mide 20 × 10 mm centrado: la mitad con color es x ∈ [−10, 0].
        for q in ink.lines.iter().flatten() {
            assert!(q.x >= -10.01 && q.x <= 0.6 && q.y.abs() <= 5.01, "{q:?}");
        }
        ink.trace.spacing_mm = 2.0;
        assert!(!ink.is_current(0.5));
    }

    fn disc(r_px: f64) -> Coverage {
        let n = (r_px * 2.0 + 10.0) as usize;
        let mut map = Grid::<f32>::new(n, n);
        let c = n as f64 / 2.0;
        for y in 0..n {
            for x in 0..n {
                if (x as f64 + 0.5 - c).hypot(y as f64 + 0.5 - c) <= r_px {
                    map.set(x, y, 0.95);
                }
            }
        }
        Coverage { map, mm_per_px: 0.25 }
    }

    fn draw(cov: &Coverage, prm: &TraceParams) -> Vec<Polyline> {
        Traced::generate(cov, &TraceKey { trace: prm.clone(), pen_width_mm: 0.5, relay: None }, 0)
    }

    #[test]
    fn old_files_keep_their_settings_and_get_the_new_ones_by_default() {
        let old: TraceParams = serde_json::from_str(r#"{"style":"Dots","spacing_mm":1.5,"angle_deg":30.0}"#).unwrap();
        assert_eq!((old.style, old.spacing_mm, old.angle_deg), (TraceStyle::Dots, 1.5, 30.0));
        assert_eq!((old.dash_mm, old.weight, old.vibe), (3.6, 0.0, Vibe::None));
    }

    #[test]
    fn every_style_and_effect_stays_inside_its_color() {
        let cov = half_map();
        for style in TraceStyle::ALL {
            for vibe in Vibe::ALL {
                let prm = TraceParams { style, vibe, vibe_mm: 1.5, vibe_by_tone: false, weight: 1.0, dash_vary: 1.0, phase: 0.3, ..Default::default() };
                let lines = draw(&cov, &prm);
                assert!(!lines.is_empty(), "{style:?} {vibe:?}");
                for q in lines.iter().flatten() {
                    // El color ocupa x ∈ [−10, 0]; el grosor añade como mucho un repaso.
                    assert!(q.x >= -10.8 && q.x <= 1.2 && q.y.abs() <= 5.8, "{style:?} {vibe:?} {q:?}");
                }
            }
        }
    }

    #[test]
    fn flow_lines_follow_the_edge_of_a_shape() {
        let cov = disc(60.0);
        let lines = draw(&cov, &TraceParams { style: TraceStyle::Flow, angle_deg: 0.0, dash_mm: 40.0, ..Default::default() });
        let (mut along, mut total) = (0usize, 0usize);
        for l in &lines {
            for w in l.windows(2) {
                let (mid, d) = (w[0].lerp(w[1], 0.5), w[1] - w[0]);
                // Cerca del borde (radio 15 mm), la línea va alrededor, no hacia fuera.
                if d.len() > 0.05 && mid.len() > 11.0 {
                    total += 1;
                    if (d.dot(mid) / (d.len() * mid.len())).abs() < 0.5 {
                        along += 1;
                    }
                }
            }
        }
        assert!(total > 20 && along * 10 >= total * 8, "{along}/{total}");
    }

    #[test]
    fn line_controls_change_lengths_thickness_and_shape() {
        let mut cov = half_map();
        cov.map.data.iter_mut().for_each(|v| *v *= 0.45);
        let lengths = |prm: &TraceParams| draw(&cov, prm).iter().map(|l| crate::geom::length(l)).collect::<Vec<_>>();
        let spread = |v: &[f64]| {
            let m = v.iter().sum::<f64>() / v.len() as f64;
            (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64).sqrt() / m
        };
        let plain = TraceParams::default();
        let long = lengths(&TraceParams { dash_mm: 12.0, ..plain.clone() });
        let short = lengths(&plain);
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        assert!(mean(&long) > mean(&short) * 1.5, "trazos más largos");
        assert!(spread(&lengths(&TraceParams { dash_vary: 1.0, ..plain.clone() })) > spread(&short) * 1.2, "largos variados");
        let ink = |prm: &TraceParams| draw(&half_map(), prm).iter().map(|l| crate::geom::length(l)).sum::<f64>();
        assert!(ink(&TraceParams { weight: 1.0, ..plain.clone() }) > ink(&plain) * 2.5, "más repasos en los oscuros");
        let wavy = TraceParams { vibe: Vibe::Wave, vibe_mm: 0.6, ..plain.clone() };
        assert!(ink(&wavy) > ink(&plain) * 1.1, "la onda alarga el recorrido");
    }

    #[test]
    fn contours_run_parallel_to_the_edge_and_spiral_and_scribble_cover_the_shape() {
        let cov = disc(60.0);
        let contours = draw(&cov, &TraceParams { style: TraceStyle::Contour, dash_mm: 60.0, ..Default::default() });
        // Cada punto de un contorno está a una distancia del borde (radio 15 mm) que es
        // múltiplo de la separación más media.
        let near_level = contours.iter().flatten().filter(|q| {
            let depth = 15.0 - q.len();
            (frac(depth / 0.9) - 0.5).abs() < 0.25
        });
        let total = contours.iter().map(|l| l.len()).sum::<usize>();
        let near_level = near_level.count();
        assert!(total > 50 && near_level * 10 >= total * 8, "{near_level}/{total}");
        for style in [TraceStyle::Spiral, TraceStyle::Scribble] {
            let lines = draw(&cov, &TraceParams { style, ..Default::default() });
            let pts: Vec<P> = lines.iter().flatten().copied().collect();
            assert!(pts.len() > 100, "{style:?}");
            // Llega a los cuatro cuadrantes del disco.
            for (sx, sy) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
                assert!(pts.iter().any(|q| q.x * sx > 5.0 && q.y * sy > 5.0), "{style:?}");
            }
        }
    }

    #[test]
    fn mixing_modes_cross_or_interleave_the_inks() {
        let mut inks = vec![TraceParams { angle_deg: 20.0, ..Default::default() }, TraceParams { style: TraceStyle::Dots, angle_deg: 70.0, ..Default::default() }, TraceParams::default()];
        mix_interleave(&mut inks);
        assert!(inks.iter().all(|t| t.style == TraceStyle::Hatch && t.angle_deg == 20.0));
        assert_eq!(inks.iter().map(|t| (t.phase * 3.0).round() as i32).collect::<Vec<_>>(), [0, 1, 2]);
        // Entrelazadas, dos tintas con todo el color no dibujan nunca la misma línea.
        let cov = half_map();
        let a = draw(&cov, &TraceParams { dash_mm: 60.0, angle_deg: 0.0, ..inks[0].clone() });
        let b = draw(&cov, &TraceParams { dash_mm: 60.0, angle_deg: 0.0, ..inks[1].clone() });
        let rows = |l: &[Polyline]| l.iter().map(|l| (l[0].y * 100.0).round() as i64).collect::<std::collections::HashSet<_>>();
        assert!(rows(&a).is_disjoint(&rows(&b)));
        mix_cross(&mut inks);
        assert_eq!(inks.iter().map(|t| t.angle_deg.round()).collect::<Vec<_>>(), [20.0, 80.0, 140.0]);
        assert!(inks.iter().all(|t| t.phase == 0.0));
        let mut own = TraceParams { angle_deg: 33.0, phase: 0.5, ..Default::default() };
        own.adopt(&TraceParams { style: TraceStyle::Flow, angle_deg: 90.0, weight: 0.4, ..Default::default() });
        assert_eq!((own.style, own.angle_deg, own.phase, own.weight), (TraceStyle::Flow, 33.0, 0.5, 0.4));
        mix_relay(&mut inks);
        assert!(inks.iter().all(|t| t.relay && t.phase == 0.0 && t.angle_deg == 20.0));
        mix_interleave(&mut inks);
        assert!(inks.iter().all(|t| !t.relay), "otra mezcla quita el relevo");
    }

    /// Dos tintas: A a la izquierda, B a la derecha y una franja central con las dos.
    fn relay_maps() -> [Coverage; 2] {
        let (mut a, mut b) = (Grid::<f32>::new(60, 20), Grid::<f32>::new(60, 20));
        for y in 0..20 {
            for x in 0..60 {
                let (va, vb) = match x { 0..20 => (0.9, 0.0), 20..40 => (0.5, 0.5), _ => (0.0, 0.9) };
                a.set(x, y, va);
                b.set(x, y, vb);
            }
        }
        [Coverage { map: a, mm_per_px: 0.5 }, Coverage { map: b, mm_per_px: 0.5 }]
    }

    #[test]
    fn relay_hands_each_line_from_one_pen_to_the_next_without_gaps_or_overlaps() {
        let maps = relay_maps();
        let inks: Vec<Shared<Coverage>> = maps.iter().cloned().map(Shared::new).collect();
        for style in [TraceStyle::Hatch, TraceStyle::Flow, TraceStyle::Contour, TraceStyle::Scribble] {
            let look = TraceParams { style, angle_deg: 0.0, dash_mm: 2.0, relay: true, ..Default::default() };
            let parts: Vec<Vec<Polyline>> = (0..2).map(|index| {
                let relay = Relay { inks: inks.clone(), index, look: look.clone(), pen_width_mm: 0.5, seed: 0 };
                Traced::generate(&maps[index], &TraceKey { trace: look.clone(), pen_width_mm: 0.5, relay: Some(relay) }, index)
            }).collect();
            // Las mismas líneas que una sola tinta con el color combinado.
            let mut combined = Grid::<f32>::new(60, 20);
            for (i, v) in combined.data.iter_mut().enumerate() {
                *v = 1.0 - (1.0 - maps[0].map.data[i]) * (1.0 - maps[1].map.data[i]);
            }
            let whole: f64 = render(&combined, 0.5, &look, 0.5, 0).iter().map(|l| crate::geom::length(l)).sum();
            let shared: f64 = parts.iter().flatten().map(|l| crate::geom::length(l)).sum();
            assert!((shared - whole).abs() < whole * 0.01, "{style:?}: {shared:.1} mm repartidos de {whole:.1} mm");
            // Cada tinta pinta donde tiene color; como mucho se pasa un tramo mínimo (0,6 mm)
            // al unir trocitos a su vecino. El centro del mapa está en el origen.
            let xs = |l: &[Polyline]| l.iter().flatten().map(|q| q.x + 15.0).fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
            assert!(xs(&parts[0]).1 < 20.25 + 0.61, "{style:?}: A se sale por la derecha {:?}", xs(&parts[0]));
            assert!(xs(&parts[1]).0 > 9.75 - 0.61, "{style:?}: B se sale por la izquierda {:?}", xs(&parts[1]));
            // En la franja central se pasan el trazo: un tramo de A acaba donde empieza uno de B.
            let ends = |l: &[Polyline]| l.iter().flat_map(|l| [l[0], *l.last().unwrap()]).collect::<Vec<_>>();
            let (ea, eb) = (ends(&parts[0]), ends(&parts[1]));
            let relays = ea.iter().filter(|q| eb.iter().any(|r| q.dist(*r) < 1e-9)).count();
            assert!(relays >= 3, "{style:?}: solo {relays} relevos");
        }
    }

    #[test]
    fn relay_dots_go_whole_to_one_ink() {
        let maps = relay_maps();
        let inks: Vec<Shared<Coverage>> = maps.iter().cloned().map(Shared::new).collect();
        let look = TraceParams { style: TraceStyle::Dots, relay: true, ..Default::default() };
        let parts: Vec<Vec<Polyline>> = (0..2).map(|index| {
            let relay = Relay { inks: inks.clone(), index, look: look.clone(), pen_width_mm: 0.5, seed: 0 };
            render_relay(&relay, &look, 0.5)
        }).collect();
        let mut combined = Grid::<f32>::new(60, 20);
        for (i, v) in combined.data.iter_mut().enumerate() {
            *v = 1.0 - (1.0 - maps[0].map.data[i]) * (1.0 - maps[1].map.data[i]);
        }
        let all = render(&combined, 0.5, &look, 0.5, 0);
        assert_eq!(parts[0].len() + parts[1].len(), all.len());
        assert!(!parts[0].is_empty() && !parts[1].is_empty());
    }
}
