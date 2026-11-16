//! Tramas cruzadas recortadas por las superficies visibles; ninguna imagen intermedia.
use super::*;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorMap {
    #[default]
    Thermal,
    Gray,
    CoolWarm,
}
impl ColorMap {
    pub fn color(self, t: f64) -> [u8; 3] {
        let stops: &[[u8; 3]] = match self {
            Self::Thermal => &[
                [53, 33, 108],
                [20, 100, 200],
                [20, 175, 120],
                [245, 125, 35],
                [250, 230, 100],
            ],
            Self::Gray => &[[20, 20, 25], [255, 255, 255]],
            Self::CoolWarm => &[[35, 80, 190], [255, 255, 255], [200, 50, 45]],
        };
        let x = t.clamp(0., 1.) * (stops.len() - 1) as f64;
        let i = (x.floor() as usize).min(stops.len() - 2);
        let f = x - i as f64;
        std::array::from_fn(|c| {
            (stops[i][c] as f64 * (1. - f) + stops[i + 1][c] as f64 * f).round() as u8
        })
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpticalInk {
    pub color: [u8; 3],
    pub width_mm: f64,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpticalSettings {
    pub colors: ColorMap,
    pub inks: Vec<OpticalInk>,
}
impl OpticalSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.inks.len() > 16
            || self
                .inks
                .iter()
                .any(|p| !p.width_mm.is_finite() || !(0.01..=10.).contains(&p.width_mm))
        {
            Err("Grosor o número de tintas ópticas no válido.".into())
        } else {
            Ok(())
        }
    }
    fn inks(&self, map: &ReadingMap) -> Vec<OpticalInk> {
        if self.inks.len() == map.pens.len() {
            self.inks.clone()
        } else {
            let pens = crate::doc::default_pens();
            map.pens
                .iter()
                .map(|i| pens.get(*i).unwrap_or(&pens[0]))
                .map(|p| OpticalInk {
                    color: p.color,
                    width_mm: p.width_mm,
                })
                .collect()
        }
    }
}
pub(super) struct Table {
    pub inks: Vec<OpticalInk>,
    rows: Vec<Vec<f64>>,
}
impl Table {
    pub fn new(map: &ReadingMap, cancel: &dyn Fn() -> bool) -> Result<Self, String> {
        let inks = map.optical.inks(map);
        let colors = inks.iter().map(|p| p.color).collect::<Vec<_>>();
        let mut rows = Vec::new();
        for i in 0..=128 {
            if cancel() {
                return Err("Cálculo cancelado.".into());
            }
            rows.push(crate::mix::color_coverage(
                map.target_color(i as f64 / 128.),
                &colors,
            ));
        }
        Ok(Self { inks, rows })
    }
    pub fn coverage(&self, slot: usize, t: f64) -> f64 {
        let x = t.clamp(0., 1.) * 128.;
        let i = (x.floor() as usize).min(127);
        let f = x - i as f64;
        self.rows[i][slot] * (1. - f) + self.rows[i + 1][slot] * f
    }
}
fn positive(lo: &mut f64, hi: &mut f64, a: f64, b: f64) -> bool {
    let delta = b - a;
    if delta.abs() < 1e-14 {
        return a >= 0.;
    }
    let t = -a / delta;
    if delta > 0. {
        *lo = lo.max(t)
    } else {
        *hi = hi.min(t)
    }
    *hi > *lo
}
pub(super) fn draw(
    surface: &surface::Surface,
    d: &Drawing,
    values: &fields::Values,
    geometry: &fields::Geometry,
    paper: &crate::Paper,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Polyline>>, String> {
    let map = &values.config;
    let table = Table::new(map, cancel)?;
    let mut out = vec![Vec::new(); map.pens.len()];
    let mut budget = 0usize;
    for slot in 0..out.len() {
        let width = table.inks[slot].width_mm;
        let gap = width.max(map.spacing_mm * 0.2).max(0.1);
        let angle = map.angle + 180. * slot as f64 / out.len() as f64;
        let dir = p(angle.to_radians().cos(), angle.to_radians().sin());
        let period = (width * 8.).max(map.spacing_mm * 2.);
        for (index, line) in effects::grid(effects::area(paper), gap, angle, cancel)?
            .into_iter()
            .enumerate()
        {
            if cancel() {
                return Err("Cálculo cancelado.".into());
            }
            for pair in line.windows(2) {
                for segment in surface.segments(pair[0], pair[1], values, geometry, &d.mesh) {
                    let a = segment.a.point;
                    let b = segment.b.point;
                    let length = a.dist(b);
                    let u0 = a.dot(dir);
                    let u1 = b.dot(dir);
                    let delta = u1 - u0;
                    if delta.abs() < 1e-12 {
                        continue;
                    }
                    let phase = encode::rank(index + slot * 7) * period;
                    let mut cuts = vec![0., 1.];
                    let count = (length / 0.75).ceil() as usize;
                    for j in 1..count {
                        cuts.push(j as f64 / count as f64);
                    }
                    let low = u0.min(u1);
                    let high = u0.max(u1);
                    for k in ((low - phase) / period * 2.).floor() as i64
                        ..=((high - phase) / period * 2.).ceil() as i64
                    {
                        let t = (phase + k as f64 * period * 0.5 - u0) / delta;
                        if t > 0. && t < 1. {
                            cuts.push(t);
                        }
                    }
                    cuts.sort_by(f64::total_cmp);
                    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
                    for w in cuts.windows(2) {
                        budget += 1;
                        if budget > 2_000_000 {
                            return Err("La mezcla supera el detalle permitido; aumenta la separación o reduce las tintas.".into());
                        }
                        if budget % 1024 == 0 && cancel() {
                            return Err("Cálculo cancelado.".into());
                        }
                        let f = |t: f64| {
                            let value = segment.a.value + (segment.b.value - segment.a.value) * t;
                            let cov = (table.coverage(slot, values.normalize(value)) * gap / width)
                                .clamp(0., 1.);
                            let u = u0 + delta * t;
                            let center = phase
                                + (((u0 + delta * (w[0] + w[1]) * 0.5) - phase) / period).round()
                                    * period;
                            cov * period * 0.5 - (u - center).abs()
                        };
                        let (mut lo, mut hi) = (0., 1.);
                        if positive(&mut lo, &mut hi, f(w[0]), f(w[1])) {
                            let start = a.lerp(b, w[0] + (w[1] - w[0]) * lo);
                            let end = a.lerp(b, w[0] + (w[1] - w[0]) * hi);
                            encode::push(&mut out[slot], start, end);
                        }
                    }
                }
            }
            if out[slot].len() > 250_000 {
                return Err("La mezcla supera 250.000 trazos.".into());
            }
        }
    }
    Ok(out)
}
pub(super) fn annotation_slot(map: &ReadingMap) -> usize {
    map.optical
        .inks(map)
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| lightness(a.color).total_cmp(&lightness(b.color)))
        .map_or(0, |(i, _)| i)
}
/// Cartela con la misma separación de tintas que el campo, no bloques de tintas sueltas.
pub(super) fn legend(
    out: &mut [Vec<Polyline>],
    map: &ReadingMap,
    values: &fields::Values,
    x: f64,
    y: f64,
    width: f64,
) {
    let annotation = annotation_slot(map);
    let Ok(table) = Table::new(map, &|| false) else {
        return;
    };
    for j in 0..64 {
        let t = j as f64 / 63.;
        let left = x + j as f64 * width / 64.;
        let right = x + (j + 1) as f64 * width / 64.;
        for (slot, lines) in out.iter_mut().enumerate() {
            let coverage = table.coverage(slot, t);
            for row in 0..12 {
                if encode::rank(row + slot * 7) < coverage {
                    lines.push(vec![
                        p(left, y + 4. + row as f64 * 0.22),
                        p(right, y + 4. + row as f64 * 0.22),
                    ]);
                }
            }
        }
    }
    for t in [0., 0.5] {
        super::legend::text(
            &mut out[annotation],
            &format!("{:.2}", values.raw_at(t)),
            p(x + t * width, y + 7.),
            1.5,
            18.,
        );
    }
}
