//! Sólidos cerrados con caras planas simples, convexas o no. Cada cara no convexa
//! se parte en trozos convexos que solo usan sus propios vértices; el motor no
//! dibuja la arista entre trozos coplanarios, así que la cara se lee entera.
use super::{normal, Scene, V3};
use crate::geom::{p, P};

/// Tolerancia para fundir vértices repetidos, en metros.
const SAME_POINT_M: f64 = 1e-9;

impl Scene {
    /// Poliedro cerrado a partir de sus caras lógicas: polígonos planos y simples,
    /// convexos o no, que comparten aristas enteras (sin vértices en T). El giro de
    /// las caras puede venir mezclado; se orientan entre sí y hacia fuera.
    /// Devuelve, por cada cara de entrada, sus trozos convexos exteriores, o nada
    /// si el poliedro no es válido. Respeta `set_quiet`, como `extrude`.
    pub fn polyhedron(&mut self, faces: &[Vec<V3>]) -> Vec<Vec<Vec<V3>>> {
        let Some((vertices, logical)) = index_faces(faces) else { return Vec::new() };
        let Some(logical) = orient(&vertices, logical) else { return Vec::new() };
        let mut pieces = Vec::new();
        let mut owner = Vec::new();
        for (i, face) in logical.iter().enumerate() {
            let Some(parts) = convex_parts(&vertices, face) else { return Vec::new() };
            for part in parts {
                pieces.push(part);
                owner.push(i);
            }
        }
        let built = self.solid_oriented(&vertices, &pieces, &[], self.quiet);
        if built.len() != pieces.len() { return Vec::new(); }
        let mut groups = vec![Vec::new(); logical.len()];
        for (face, i) in built.into_iter().zip(owner) { groups[i].push(face); }
        groups
    }

    /// Prisma recto de un perfil plano simple, convexo o no (una escalera, una casa
    /// con su tejado). Devuelve las caras agrupadas: primero la tapa del perfil,
    /// después la tapa desplazada y luego un lateral por cada lado del perfil.
    pub fn prism(&mut self, profile: &[V3], vector: V3) -> Vec<Vec<Vec<V3>>> {
        if profile.len() < 3 || !vector.finite() || !profile.iter().all(|q| q.finite()) { return Vec::new(); }
        if normal(profile).dot(vector).abs() <= 1e-12 * vector.length().max(1e-300) { return Vec::new(); }
        let n = profile.len();
        let mut faces = vec![profile.to_vec(), profile.iter().map(|q| *q + vector).collect()];
        for i in 0..n {
            let j = (i + 1) % n;
            faces.push(vec![profile[i], profile[j], profile[j] + vector, profile[i] + vector]);
        }
        self.polyhedron(&faces)
    }
}

/// Funde los vértices repetidos y describe cada cara por índices.
fn index_faces(faces: &[Vec<V3>]) -> Option<(Vec<V3>, Vec<Vec<usize>>)> {
    if faces.len() < 4 { return None; }
    let mut vertices: Vec<V3> = Vec::new();
    let mut logical = Vec::with_capacity(faces.len());
    for face in faces {
        if face.len() < 3 || !face.iter().all(|q| q.finite()) { return None; }
        let mut indices: Vec<usize> = Vec::with_capacity(face.len());
        for q in face {
            let found = vertices.iter().position(|v| (*v - *q).length() <= SAME_POINT_M);
            let index = found.unwrap_or_else(|| {
                vertices.push(*q);
                vertices.len() - 1
            });
            if indices.last() != Some(&index) { indices.push(index); }
        }
        if indices.len() > 1 && indices[0] == indices[indices.len() - 1] { indices.pop(); }
        if indices.len() < 3 { return None; }
        logical.push(indices);
    }
    Some((vertices, logical))
}

/// Da a todas las caras un giro coherente (cada arista compartida se recorre en
/// sentidos opuestos) y, si el volumen sale negativo, les da la vuelta a todas.
fn orient(vertices: &[V3], mut faces: Vec<Vec<usize>>) -> Option<Vec<Vec<usize>>> {
    let edges = |face: &Vec<usize>| -> Vec<(usize, usize)> {
        (0..face.len()).map(|k| (face[k], face[(k + 1) % face.len()])).collect()
    };
    let mut done = vec![false; faces.len()];
    let mut queue = vec![0];
    done[0] = true;
    while let Some(i) = queue.pop() {
        let mine = edges(&faces[i]);
        for j in 0..faces.len() {
            if done[j] { continue; }
            let theirs = edges(&faces[j]);
            let same = theirs.iter().any(|e| mine.contains(e));
            let opposite = theirs.iter().any(|&(a, b)| mine.contains(&(b, a)));
            if same && opposite { return None; }
            if !same && !opposite { continue; }
            if same { faces[j].reverse(); }
            done[j] = true;
            queue.push(j);
        }
    }
    // Un poliedro de una pieza alcanza todas sus caras desde la primera.
    if !done.iter().all(|d| *d) { return None; }
    let volume: f64 = faces.iter().map(|face| {
        let a = vertices[face[0]];
        (1..face.len() - 1).map(|k| a.dot(vertices[face[k]].cross(vertices[face[k + 1]]))).sum::<f64>()
    }).sum();
    if volume.abs() < 1e-15 { return None; }
    if volume < 0.0 { for face in &mut faces { face.reverse(); } }
    Some(faces)
}

/// Coordenadas en el plano de la cara, con el giro exterior en sentido antihorario.
fn flatten(vertices: &[V3], face: &[usize]) -> Option<Vec<P>> {
    let points: Vec<V3> = face.iter().map(|&i| vertices[i]).collect();
    // Normal de Newell: robusta con vértices alineados y caras no convexas.
    let mut n = V3::default();
    for k in 0..points.len() {
        let (a, b) = (points[k], points[(k + 1) % points.len()]);
        n = n + V3::new((a.y - b.y) * (a.z + b.z), (a.z - b.z) * (a.x + b.x), (a.x - b.x) * (a.y + b.y));
    }
    if n.length() < 1e-15 { return None; }
    let n = n.normalized();
    if !points.iter().all(|q| (*q - points[0]).dot(n).abs() <= 1e-8) { return None; }
    let helper = if n.x.abs() < 0.9 { V3::new(1.0, 0.0, 0.0) } else { V3::new(0.0, 1.0, 0.0) };
    let u = helper.cross(n).normalized();
    let v = n.cross(u);
    Some(points.iter().map(|q| p((*q - points[0]).dot(u), (*q - points[0]).dot(v))).collect())
}

fn turn(a: P, b: P, c: P) -> f64 { super::cross(b - a, c - b) }

fn convex(points: &[P], cycle: &[usize], tolerance: f64) -> bool {
    let n = cycle.len();
    (0..n).all(|k| turn(points[cycle[k]], points[cycle[(k + 1) % n]], points[cycle[(k + 2) % n]]) >= -tolerance)
}

/// Punto dentro del triángulo antihorario o sobre su borde.
fn inside(q: P, a: P, b: P, c: P, tolerance: f64) -> bool {
    super::cross(b - a, q - a) >= -tolerance && super::cross(c - b, q - b) >= -tolerance && super::cross(a - c, q - c) >= -tolerance
}

/// Trozos convexos de una cara: recorte de orejas y después se funden triángulos
/// vecinos mientras el resultado siga siendo convexo (Hertel-Mehlhorn).
fn convex_parts(vertices: &[V3], face: &[usize]) -> Option<Vec<Vec<usize>>> {
    let points = flatten(vertices, face)?;
    let size = points.iter().fold(0.0f64, |m, q| m.max(q.x.abs()).max(q.y.abs())).max(1e-9);
    let tolerance = size * size * 1e-12;
    let local: Vec<usize> = (0..face.len()).collect();
    if convex(&points, &local, tolerance) { return Some(vec![face.to_vec()]); }
    let mut parts = ears(&points, tolerance)?;
    merge(&points, &mut parts, tolerance);
    Some(parts.into_iter().map(|part| part.into_iter().map(|k| face[k]).collect()).collect())
}

fn ears(points: &[P], tolerance: f64) -> Option<Vec<Vec<usize>>> {
    let mut left: Vec<usize> = (0..points.len()).collect();
    let mut triangles = Vec::new();
    while left.len() > 3 {
        let n = left.len();
        let ear = (0..n).find(|&k| {
            let (a, b, c) = (left[(k + n - 1) % n], left[k], left[(k + 1) % n]);
            if turn(points[a], points[b], points[c]) <= tolerance { return false; }
            left.iter().all(|&o| o == a || o == b || o == c || points[o] == points[a] || points[o] == points[c] ||
                !inside(points[o], points[a], points[b], points[c], tolerance))
        })?;
        triangles.push(vec![left[(ear + n - 1) % n], left[ear], left[(ear + 1) % n]]);
        left.remove(ear);
    }
    if turn(points[left[0]], points[left[1]], points[left[2]]) <= tolerance { return None; }
    triangles.push(left);
    Some(triangles)
}

fn merge(points: &[P], parts: &mut Vec<Vec<usize>>, tolerance: f64) {
    'search: loop {
        for i in 0..parts.len() {
            for j in i + 1..parts.len() {
                let Some(joined) = join(&parts[i], &parts[j]) else { continue };
                if !convex(points, &joined, tolerance) { continue; }
                parts[i] = joined;
                parts.remove(j);
                continue 'search;
            }
        }
        return;
    }
}

/// Une dos ciclos que comparten una arista recorrida en sentidos opuestos.
fn join(a: &[usize], b: &[usize]) -> Option<Vec<usize>> {
    for k in 0..a.len() {
        let (u, v) = (a[k], a[(k + 1) % a.len()]);
        let Some(m) = (0..b.len()).find(|&m| b[m] == v && b[(m + 1) % b.len()] == u) else { continue };
        // a desde v hasta u, y b desde u hasta justo antes de v.
        let mut cycle: Vec<usize> = (0..a.len()).map(|t| a[(k + 1 + t) % a.len()]).collect();
        cycle.extend((2..b.len()).map(|t| b[(m + t) % b.len()]));
        return Some(cycle);
    }
    None
}

#[cfg(test)]
impl Scene {
    /// Costuras: trazos dibujados a lo largo del borde común de dos caras visibles,
    /// coplanarias y con la misma normal. Entre sólidos distintos (casas adosadas)
    /// son juntas legítimas; dentro de una pieza, un defecto.
    pub(crate) fn seams(&self, output: &super::SceneOutput) -> Vec<String> {
        const LONG_MM: f64 = 0.1;
        const NEAR_MM: f64 = 1e-4;
        let visible: Vec<&super::hidden::Face> = self.faces.iter().filter(|f| self.camera.facing(f.normal)).collect();
        let lines: Vec<(P, P)> = output.parts.iter().flat_map(|part| &part.lines)
            .flat_map(|line| line.windows(2).map(|w| (w[0], w[1]))).collect();
        let edges = |face: &super::hidden::Face| -> Vec<(P, P)> {
            let n = face.points.len();
            (0..n).map(|k| (face.points[k], face.points[(k + 1) % n])).collect()
        };
        let mut issues = Vec::new();
        for (i, a) in visible.iter().enumerate() {
            for b in &visible[i + 1..] {
                if a.normal.dot(b.normal) < 1.0 - 1e-9 || (b.world_origin - a.world_origin).dot(a.normal).abs() > 1e-6 { continue; }
                for (u, v) in edges(a) {
                    let length = u.dist(v);
                    if length < LONG_MM { continue; }
                    let unit = (v - u) * (1.0 / length);
                    let along = |q: P| (q - u).dot(unit);
                    let on_line = |q: P| super::cross(unit, q - u).abs() < NEAR_MM;
                    for (s, t) in edges(b) {
                        if !on_line(s) || !on_line(t) { continue; }
                        let (lo, hi) = (along(s).min(along(t)).max(0.0), along(s).max(along(t)).min(length));
                        if hi - lo < LONG_MM { continue; }
                        let drawn = lines.iter().any(|&(x, y)| on_line(x) && on_line(y) &&
                            along(x).max(along(y)).min(hi) - along(x).min(along(y)).max(lo) >= LONG_MM);
                        if drawn { issues.push(format!("Costura de {:.2} mm entre {:?} y {:?}", hi - lo, a.solid_id, b.solid_id)); }
                    }
                }
            }
        }
        issues
    }
}
