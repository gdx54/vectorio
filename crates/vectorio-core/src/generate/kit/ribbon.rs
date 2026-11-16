//! Cintas curvas: calles, circuitos y vías a partir de una polilínea suave.
//!
//! La banda se calcula una vez (`Band`) y sirve a la vez para dibujarla, para
//! colocar vehículos y gente a lo largo de ella y para medir su longitud.
use super::*;

/// Marcas sobre la cinta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marking {
    /// Solo los bordes.
    Plain,
    /// Línea central discontinua.
    Dashed,
    /// Pianos de circuito en los dos bordes.
    Kerbs,
    /// Dos carriles y traviesas.
    Rails,
}

/// Suaviza una polilínea abierta (Chaikin), conservando sus extremos.
pub fn smooth(points: &[V3], rounds: usize) -> Vec<V3> {
    let mut current: Vec<V3> = points.iter().copied().filter(|q| q.finite()).collect();
    for _ in 0..rounds.min(8) {
        if current.len() < 3 {
            break;
        }
        let mut next = vec![current[0]];
        for pair in current.windows(2) {
            next.push(pair[0] * 0.75 + pair[1] * 0.25);
            next.push(pair[0] * 0.25 + pair[1] * 0.75);
        }
        next.push(current[current.len() - 1]);
        current = next;
    }
    current
}

/// Reparte puntos a distancia `step` a lo largo de la polilínea; conserva los extremos.
pub fn resample(points: &[V3], step: f64) -> Vec<V3> {
    if points.len() < 2 || !step.is_finite() || step <= 0.0 {
        return points.to_vec();
    }
    let total: f64 = points.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    if !total.is_finite() || total <= 0.0 {
        return points.to_vec();
    }
    let count = ((total / step).ceil() as usize).clamp(1, 20_000);
    (0..=count).map(|k| point_at(points, total * k as f64 / count as f64)).collect()
}

/// Punto a la distancia `s` del principio, medida sobre la polilínea.
fn point_at(points: &[V3], s: f64) -> V3 {
    let mut left = s.max(0.0);
    for pair in points.windows(2) {
        let length = (pair[1] - pair[0]).length();
        if left <= length && length > 0.0 {
            return pair[0] + (pair[1] - pair[0]) * (left / length);
        }
        left -= length;
    }
    points[points.len() - 1]
}

/// Banda de anchura `width` alrededor de una polilínea. La sección es horizontal:
/// la cinta sube y baja con su eje, sin peralte.
#[derive(Clone, Debug)]
pub struct Band {
    pub center: Vec<V3>,
    pub left: Vec<V3>,
    pub right: Vec<V3>,
    pub width: f64,
    /// Distancia acumulada de cada punto del eje.
    pub stations: Vec<f64>,
}

impl Band {
    /// El eje se reparte cada metro como mucho; con menos de dos puntos no hay banda.
    pub fn new(path: &[V3], width: f64) -> Option<Self> {
        if !width.is_finite() || width <= 0.0 || width > 500.0 {
            return None;
        }
        let center = resample(path, (width * 0.5).clamp(0.25, 1.0));
        if center.len() < 2 {
            return None;
        }
        let normals: Vec<V3> = (0..center.len()).map(|i| side(&center, i)).collect();
        if normals.iter().any(|n| n.length() < 0.5) {
            return None;
        }
        let left = center.iter().zip(&normals).map(|(c, n)| *c + *n * (width * 0.5)).collect();
        let right = center.iter().zip(&normals).map(|(c, n)| *c - *n * (width * 0.5)).collect();
        let mut stations = vec![0.0];
        for pair in center.windows(2) {
            let last = stations[stations.len() - 1];
            stations.push(last + (pair[1] - pair[0]).length());
        }
        Some(Self { center, left, right, width, stations })
    }
    pub fn length(&self) -> f64 {
        self.stations.last().copied().unwrap_or(0.0)
    }
    /// Línea paralela al eje a `offset` metros (positivo a la izquierda).
    pub fn offset(&self, offset: f64) -> Vec<V3> {
        (0..self.center.len()).map(|i| self.center[i] + side(&self.center, i) * offset).collect()
    }
    /// Posición y rumbo horizontal a la distancia `s`, desplazada `offset` metros.
    pub fn at(&self, s: f64, offset: f64) -> (V3, V3) {
        let s = s.clamp(0.0, self.length());
        let i = self.stations.partition_point(|t| *t < s).clamp(1, self.center.len() - 1);
        let span = (self.stations[i] - self.stations[i - 1]).max(1e-12);
        let k = ((s - self.stations[i - 1]) / span).clamp(0.0, 1.0);
        let position = self.center[i - 1] + (self.center[i] - self.center[i - 1]) * k;
        let normal = side(&self.center, i - 1) * (1.0 - k) + side(&self.center, i) * k;
        let normal = normal.normalized();
        let heading = v(normal.y, -normal.x, 0.0);
        (position + normal * offset, heading)
    }
}

/// Normal horizontal a la izquierda del eje en el punto `i`.
fn side(center: &[V3], i: usize) -> V3 {
    let a = center[i.saturating_sub(1)];
    let b = center[(i + 1).min(center.len() - 1)];
    let t = v(b.x - a.x, b.y - a.y, 0.0).normalized();
    v(-t.y, t.x, 0.0)
}

/// Dibuja la cinta. Con `deck` mayor que cero es un tablero elevado de ese canto,
/// que oculta lo que pasa por debajo.
pub fn ribbon(scene: &mut Scene, band: &Band, deck: f64, marking: Marking) {
    let n = band.center.len();
    if n < 2 || !deck.is_finite() || deck < 0.0 {
        return;
    }
    part(scene, "contorno");
    for i in 0..n - 1 {
        scene.occluder(&[band.left[i], band.right[i], band.right[i + 1]]);
        scene.occluder(&[band.left[i], band.right[i + 1], band.left[i + 1]]);
    }
    scene.line(&band.left);
    scene.line(&band.right);
    if deck > 0.0 {
        let down = v(0.0, 0.0, deck);
        for edge in [&band.left, &band.right] {
            for i in 0..n - 1 {
                scene.occluder(&[edge[i], edge[i + 1], edge[i + 1] - down, edge[i] - down]);
            }
            let under: Vec<V3> = edge.iter().map(|q| *q - down).collect();
            scene.line(&under);
        }
        for i in [0, n - 1] {
            let face = [band.left[i], band.right[i], band.right[i] - down, band.left[i] - down];
            scene.occluder(&face);
            scene.closed(&face);
        }
    }
    if scene.paper_mm(band.width) < 2.0 {
        return;
    }
    part(scene, "detalle");
    match marking {
        Marking::Plain => {}
        Marking::Dashed => dashes(scene, band, 0.0, 3.0, 3.0),
        Marking::Kerbs => kerbs(scene, band),
        Marking::Rails => rails(scene, band),
    }
}

/// Trazos de `dash` metros cada `dash + gap` a lo largo de la paralela `offset`.
fn dashes(scene: &mut Scene, band: &Band, offset: f64, dash: f64, gap: f64) {
    if !scene.visible(dash, 0.8) {
        return;
    }
    let mut s = gap * 0.5;
    while s + dash <= band.length() {
        let pieces: Vec<V3> = (0..=4).map(|k| band.at(s + dash * k as f64 / 4.0, offset).0).collect();
        scene.line(&pieces);
        s += dash + gap;
    }
}

/// Pianos: una franja de 0,8 m en cada borde con rayas transversales alternas.
fn kerbs(scene: &mut Scene, band: &Band) {
    let inner = band.width * 0.5 - 0.8f64.min(band.width * 0.2);
    for sign in [1.0, -1.0] {
        scene.line(&band.offset(inner * sign));
        if !scene.visible(1.0, 0.9) {
            continue;
        }
        let mut s = 0.5;
        while s < band.length() {
            let a = band.at(s, inner * sign).0;
            let b = band.at(s, band.width * 0.5 * sign).0;
            scene.line(&[a, b]);
            s += 1.0;
        }
    }
}

/// Vía de ancho internacional: dos carriles y traviesas cada 0,7 m (o más si no caben).
fn rails(scene: &mut Scene, band: &Band) {
    let gauge = (1.435f64 * 0.5).min(band.width * 0.35);
    scene.line(&band.offset(gauge));
    scene.line(&band.offset(-gauge));
    let sleeper = (band.width * 0.42).min(1.3);
    let step = 0.7f64.max(1.0 / scene.camera().scale);
    let mut s = step * 0.5;
    while s < band.length() {
        scene.line(&[band.at(s, sleeper).0, band.at(s, -sleeper).0]);
        s += step;
    }
}

/// Paso de cebra a la distancia `s`: franjas de 0,5 m paralelas a la cinta, de
/// `length` metros, que cubren todo su ancho.
pub fn zebra(scene: &mut Scene, band: &Band, s: f64, length: f64) {
    if !scene.visible(0.5, 0.6) || !length.is_finite() || length <= 0.0 {
        return;
    }
    part(scene, "detalle");
    let stripes = ((band.width - 0.5) / 1.0).floor().max(1.0) as usize;
    let first = -(stripes as f64 - 1.0) * 0.5;
    for k in 0..stripes {
        let offset = first + k as f64;
        let a = band.at(s, offset - 0.25).0;
        let b = band.at(s + length, offset - 0.25).0;
        let c = band.at(s + length, offset + 0.25).0;
        let d = band.at(s, offset + 0.25).0;
        scene.closed(&[a, b, c, d]);
    }
}
