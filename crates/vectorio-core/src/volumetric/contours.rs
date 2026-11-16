//! Isolíneas lineales por triángulo y ensamblaje determinista por proximidad real.
use super::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub(super) struct Welder {
    tolerance: f64,
    cells: HashMap<(i64, i64, i64), Vec<usize>>,
    pub points: Vec<V3>,
}
impl Welder {
    pub fn new(tolerance: f64) -> Self {
        Self {
            tolerance,
            cells: HashMap::new(),
            points: vec![],
        }
    }
    pub fn insert(&mut self, p: V3) -> usize {
        let key = (
            (p.x / self.tolerance).floor() as i64,
            (p.y / self.tolerance).floor() as i64,
            (p.z / self.tolerance).floor() as i64,
        );
        if let Some(ids) = self.cells.get(&key) {
            for &i in ids {
                if (self.points[i] - p).length() <= self.tolerance {
                    return i;
                }
            }
        }
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    if x == 0 && y == 0 && z == 0 {
                        continue;
                    }
                    if let Some(ids) = self.cells.get(&(key.0 + x, key.1 + y, key.2 + z)) {
                        for &i in ids {
                            if (self.points[i] - p).length() <= self.tolerance {
                                return i;
                            }
                        }
                    }
                }
            }
        }
        let i = self.points.len();
        self.points.push(p);
        self.cells.entry(key).or_default().push(i);
        i
    }
}
#[derive(Clone, Debug)]
pub struct Contour {
    pub level: usize,
    pub value: f64,
    pub points: Vec<V3>,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Field {
    Height,
    CameraDepth,
    Distance,
}

pub fn field_values(mesh: &Mesh, field: Field, direction: V3, camera: &Camera) -> Vec<f64> {
    let away = camera.axes().2;
    mesh.points()
        .into_iter()
        .map(|p| match field {
            Field::Height => p.dot(direction.normalized()),
            Field::CameraDepth => (p - camera.world_center).dot(away),
            Field::Distance => p.length(),
        })
        .collect()
}
/// Incluye bordes de regiones coplanarias, elimina sus diagonales internas y no
/// fuerza una unión a través de ramificaciones no manifold.
pub fn isolines(
    mesh: &Mesh,
    values: &[f64],
    levels: &[f64],
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Contour>, String> {
    if values.len() != mesh.vertices.len()
        || values.iter().chain(levels).any(|v| !v.is_finite())
        || levels.len() > 160
    {
        return Err("Campo escalar no válido o demasiados niveles.".into());
    }
    let p = mesh.points();
    let mut output = Vec::new();
    let mut segments = 0;
    let range = values
        .iter()
        .copied()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
            (a.min(x), b.max(x))
        });
    let eps = ((range.1 - range.0) * 1e-10).max(1e-10);
    for (level, &value) in levels.iter().enumerate() {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let mut weld = Welder::new(1e-7);
        let mut edges = BTreeSet::new();
        let mut coplanar = BTreeMap::<(usize, usize), usize>::new();
        for (face, t) in mesh.triangles.iter().enumerate() {
            if face % 256 == 0 && cancel() {
                return Err("Cálculo cancelado.".into());
            }
            let mut t = *t;
            t.sort();
            let d = t.map(|i| values[i] - value);
            if d.iter().all(|v| v.abs() <= eps) {
                for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                    let a = weld.insert(p[t[a]]);
                    let b = weld.insert(p[t[b]]);
                    if a != b {
                        *coplanar.entry((a.min(b), a.max(b))).or_default() += 1;
                    }
                }
                continue;
            }
            let mut hits = Vec::new();
            for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                if d[a].abs() <= eps {
                    hits.push(weld.insert(p[t[a]]));
                }
                if (d[a] > eps && d[b] < -eps) || (d[a] < -eps && d[b] > eps) {
                    let u = d[a] / (d[a] - d[b]);
                    hits.push(weld.insert(p[t[a]] + (p[t[b]] - p[t[a]]) * u));
                }
            }
            hits.sort();
            hits.dedup();
            if hits.len() == 2 {
                edges.insert((hits[0], hits[1]));
            }
        }
        edges.extend(
            coplanar
                .into_iter()
                .filter(|(_, n)| *n == 1)
                .map(|(edge, _)| edge),
        );
        segments += edges.len();
        if segments > 2_000_000 {
            return Err(
                "Más de dos millones de segmentos: reduce los cortes o selecciona una calidad de malla más ligera.".into(),
            );
        }
        let mut adjacency = vec![Vec::<(usize, usize)>::new(); weld.points.len()];
        let edges: Vec<_> = edges.into_iter().collect();
        for (i, &(a, b)) in edges.iter().enumerate() {
            adjacency[a].push((b, i));
            adjacency[b].push((a, i));
        }
        let mut used = vec![false; edges.len()];
        // Primero extremos y bifurcaciones; después los ciclos de grado dos.
        let order = adjacency
            .iter()
            .enumerate()
            .filter(|(_, e)| e.len() != 2)
            .map(|(i, _)| i)
            .chain(0..adjacency.len());
        for start in order {
            for &(_, edge) in &adjacency[start] {
                if used[edge] {
                    continue;
                }
                let mut path = vec![weld.points[start]];
                let mut at = start;
                let mut next = edge;
                loop {
                    used[next] = true;
                    let (a, b) = edges[next];
                    at = if at == a { b } else { a };
                    path.push(weld.points[at]);
                    if at == start || adjacency[at].len() != 2 {
                        break;
                    }
                    if let Some((_, e)) = adjacency[at].iter().find(|(_, e)| !used[*e]) {
                        next = *e;
                    } else {
                        break;
                    }
                }
                output.push(Contour {
                    level,
                    value,
                    points: path,
                });
            }
        }
    }
    Ok(output)
}
