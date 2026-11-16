//! Malla de origen independiente de cámara, papel y tintas.
use super::*;
use std::collections::{HashMap, HashSet};

pub const MAX_VERTICES: usize = 15_000_000;
pub const MAX_FACES: usize = 5_000_000;
pub const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_RECIPE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mesh {
    pub name: String,
    pub vertices: Vec<[f64; 3]>,
    pub triangles: Vec<[usize; 3]>,
    /// OBJ conserva nombres y pertenencia, sin confundir objetos con tintas.
    #[serde(default)]
    pub objects: Vec<String>,
    #[serde(default)]
    pub face_objects: Vec<usize>,
}
#[derive(Clone, Debug, Default)]
pub struct MeshReport {
    pub removed: usize,
    pub boundary_edges: usize,
    pub non_manifold_edges: usize,
}
impl Mesh {
    pub fn points(&self) -> Vec<V3> {
        self.vertices.iter().map(|q| v3(*q)).collect()
    }
    /// Valida antes de indexar. Desecha degenerados y caras repetidas; no intenta
    /// reparar agujeros ni invertir automáticamente las normales del artista.
    pub fn validated(self) -> Result<(Self, MeshReport), String> {
        self.validated_cancellable(&|| false)
    }
    pub(super) fn validated_cancellable(
        mut self,
        cancel: &dyn Fn() -> bool,
    ) -> Result<(Self, MeshReport), String> {
        if self.vertices.is_empty()
            || self.vertices.len() > MAX_VERTICES
            || self.triangles.len() > MAX_FACES
        {
            return Err(
                "El modelo admite hasta 15 millones de vértices y 5 millones de triángulos.".into(),
            );
        }
        if self.objects.len() > MAX_FACES
            || self.name.len() > 4096
            || self.objects.iter().any(|s| s.len() > 4096)
        {
            return Err("Los nombres del modelo superan el límite permitido.".into());
        }
        if self
            .vertices
            .iter()
            .flatten()
            .any(|n| !n.is_finite() || n.abs() > 1e12)
        {
            return Err("El modelo contiene coordenadas no finitas o fuera de rango.".into());
        }
        if self
            .triangles
            .iter()
            .flatten()
            .any(|i| *i >= self.vertices.len())
        {
            return Err("Una cara referencia un vértice inexistente.".into());
        }
        if !self.face_objects.is_empty()
            && (self.face_objects.len() != self.triangles.len()
                || self.face_objects.iter().any(|i| *i >= self.objects.len()))
        {
            return Err("La pertenencia de las caras a los objetos no es válida.".into());
        }
        // Centrar antes de cuantizar: modelos lejanos al origen no desbordan la rejilla.
        let bounds = self.bounds();
        let center = (bounds[0] + bounds[1]) * 0.5;
        let size = (bounds[1] - bounds[0]).length();
        if !size.is_finite() || size < 1e-12 {
            return Err("El modelo no tiene extensión geométrica.".into());
        }
        let points = self.points();
        let mut weld = super::contours::Welder::new(1e-8);
        let mut ids = Vec::with_capacity(points.len());
        let mut compact = Vec::new();
        for (i, q) in points.iter().enumerate() {
            if i % 4096 == 0 && cancel() {
                return Err("Cálculo cancelado.".into());
            }
            let id = weld.insert((*q - center) * (1.0 / size));
            if id == compact.len() {
                compact.push(self.vertices[i]);
            }
            ids.push(id);
        }
        drop(weld);
        let mut seen = HashSet::new();
        let mut edges = HashMap::<(usize, usize), usize>::new();
        let mut faces = Vec::new();
        let mut owners = Vec::new();
        let mut report = MeshReport::default();
        for (i, t) in self.triangles.iter().enumerate() {
            if i % 4096 == 0 && cancel() {
                return Err("Cálculo cancelado.".into());
            }
            let mut key = t.map(|j| ids[j]);
            key.sort();
            let area = (points[t[1]] - points[t[0]])
                .cross(points[t[2]] - points[t[0]])
                .length()
                / (size * size);
            if key[0] == key[1] || key[1] == key[2] || area < 1e-14 || !seen.insert(key) {
                report.removed += 1;
                continue;
            }
            faces.push(t.map(|j| ids[j]));
            if !self.face_objects.is_empty() {
                owners.push(self.face_objects[i]);
            }
            for (a, b) in [
                (ids[t[0]], ids[t[1]]),
                (ids[t[1]], ids[t[2]]),
                (ids[t[2]], ids[t[0]]),
            ] {
                *edges.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        report.boundary_edges = edges.values().filter(|n| **n == 1).count();
        report.non_manifold_edges = edges.values().filter(|n| **n > 2).count();
        if faces.is_empty() {
            return Err("El modelo no contiene triángulos utilizables.".into());
        }
        self.vertices = compact;
        self.triangles = faces;
        self.face_objects = owners;
        Ok((self, report))
    }
    pub fn bounds(&self) -> [V3; 2] {
        let mut lo = V3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut hi = lo * (-1.0);
        for p in self.vertices.iter().copied().map(v3) {
            lo = V3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = V3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
        [lo, hi]
    }
    /// Normalización explícita a dos unidades en el eje mayor, Z arriba. El modelo
    /// original sigue en la receta; sus unidades OBJ nunca se interpretan como mm.
    pub fn transformed(&self, scale: [f64; 3], rotation: [f64; 3]) -> Self {
        let [lo, hi] = self.bounds();
        let c = (lo + hi) * 0.5;
        let d = hi - lo;
        let k = 2.0 / d.x.max(d.y).max(d.z).max(1e-12);
        let vertices = self
            .points()
            .into_iter()
            .map(|p| {
                let q = (p - c) * k;
                let mut q = V3::new(q.x * scale[0], q.y * scale[1], q.z * scale[2]);
                let (s, c) = rotation[0].to_radians().sin_cos();
                q = V3::new(q.x, c * q.y - s * q.z, s * q.y + c * q.z);
                let (s, c) = rotation[1].to_radians().sin_cos();
                q = V3::new(c * q.x + s * q.z, q.y, -s * q.x + c * q.z);
                let (s, c) = rotation[2].to_radians().sin_cos();
                arr(V3::new(c * q.x - s * q.y, s * q.x + c * q.y, q.z))
            })
            .collect();
        Self {
            vertices,
            name: self.name.clone(),
            triangles: self.triangles.clone(),
            objects: self.objects.clone(),
            face_objects: self.face_objects.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Primitive {
    Cube,
    Sphere,
    Cylinder,
    Cone,
    Plane,
    Torus,
    Assembly,
}
impl Primitive {
    pub fn mesh(self) -> Mesh {
        let mut m = Mesh {
            name: match self {
                Self::Cube => "Cubo",
                Self::Sphere => "Esfera",
                Self::Cylinder => "Cilindro",
                Self::Cone => "Cono",
                Self::Plane => "Plano",
                Self::Torus => "Toroide",
                Self::Assembly => "Composición de volúmenes",
            }
            .into(),
            vertices: vec![],
            triangles: vec![],
            objects: vec![],
            face_objects: vec![],
        };
        if self == Self::Cube || self == Self::Assembly {
            m.vertices = vec![
                [-1., -1., -1.],
                [1., -1., -1.],
                [1., 1., -1.],
                [-1., 1., -1.],
                [-1., -1., 1.],
                [1., -1., 1.],
                [1., 1., 1.],
                [-1., 1., 1.],
            ];
            for [a, b, c, d] in [
                [0, 3, 2, 1],
                [4, 5, 6, 7],
                [0, 1, 5, 4],
                [1, 2, 6, 5],
                [2, 3, 7, 6],
                [3, 0, 4, 7],
            ] {
                m.triangles.extend([[a, b, c], [a, c, d]]);
            }
            if self == Self::Assembly {
                let original = m.clone();
                m.objects = vec![
                    "Prisma bajo".into(),
                    "Torre".into(),
                    "Bloque suspendido".into(),
                ];
                m.vertices.clear();
                m.triangles.clear();
                for (owner, (s, t)) in [
                    ([1.0, 0.55, 0.28], [-0.2, 0., -0.7]),
                    ([0.4, 0.4, 1.0], [0.45, 0.1, 0.]),
                    ([0.65, 0.3, 0.22], [-0.3, 0., 0.65]),
                ]
                .into_iter()
                .enumerate()
                {
                    let offset = m.vertices.len();
                    m.vertices.extend(
                        original
                            .vertices
                            .iter()
                            .map(|p| [p[0] * s[0] + t[0], p[1] * s[1] + t[1], p[2] * s[2] + t[2]]),
                    );
                    m.triangles
                        .extend(original.triangles.iter().map(|f| f.map(|i| i + offset)));
                    m.face_objects
                        .extend(std::iter::repeat_n(owner, original.triangles.len()));
                }
            }
        } else if self == Self::Plane {
            m.vertices = vec![[-1., -1., 0.], [1., -1., 0.], [1., 1., 0.], [-1., 1., 0.]];
            m.triangles = vec![[0, 1, 2], [0, 2, 3]];
        } else {
            let n = 64;
            let rows = if self == Self::Torus {
                24
            } else if self == Self::Sphere {
                32
            } else {
                1
            };
            for j in 0..=rows {
                for i in 0..n {
                    let u = i as f64 / n as f64 * std::f64::consts::TAU;
                    let v = j as f64 / rows as f64;
                    let (r, z) = match self {
                        Self::Sphere => {
                            let a = v * std::f64::consts::PI;
                            (a.sin(), a.cos())
                        }
                        Self::Torus => {
                            let a = v * std::f64::consts::TAU;
                            (0.72 + 0.28 * a.cos(), 0.28 * a.sin())
                        }
                        Self::Cone => (1.0 - v, 2.0 * v - 1.0),
                        _ => (1., 2.0 * v - 1.),
                    };
                    m.vertices.push([r * u.cos(), r * u.sin(), z]);
                }
            }
            for j in 0..rows {
                for i in 0..n {
                    let a = j * n + i;
                    let b = j * n + (i + 1) % n;
                    let c = b + n;
                    let d = a + n;
                    m.triangles.extend([[a, b, c], [a, c, d]]);
                }
            }
            if matches!(self, Self::Cylinder | Self::Cone) {
                for j in [0, rows] {
                    let center = m.vertices.len();
                    m.vertices.push([0., 0., if j == 0 { -1. } else { 1. }]);
                    for i in 0..n {
                        m.triangles.push(if j == 0 {
                            [center, j * n + (i + 1) % n, j * n + i]
                        } else {
                            [center, j * n + i, j * n + (i + 1) % n]
                        });
                    }
                }
            }
        }
        if self == Self::Sphere {
            for t in &mut m.triangles {
                t.swap(0, 1);
            }
        }
        m.validated().expect("primitiva válida").0
    }
}
