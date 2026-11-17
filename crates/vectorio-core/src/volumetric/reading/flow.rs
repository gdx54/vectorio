//! Principal-curvature streamlines. Integration stays on adjacent triangles;
//! object-space placement survives camera orbit and has no raster intermediate.
use super::*;
use std::collections::HashMap;

struct Surface<'a> {
    shape: &'a Shape,
    mesh: &'a Mesh,
    directions: Vec<Option<V3>>,
    normals: Vec<V3>,
    adjacent: Vec<[Option<usize>; 3]>,
    components: Vec<usize>,
}
fn principal([a, b, c]: [f64; 3], [x, y]: [V3; 2], minimum: bool) -> Option<V3> {
    let gap = ((a - c) * (a - c) + 4. * b * b).sqrt();
    let scale = ((a + c).abs() + gap) * 0.5;
    if scale < 1e-10 || gap < scale * 0.08 {
        return None;
    }
    let angle = 0.5 * (2. * b).atan2(a - c);
    let (s, c) = angle.sin_cos();
    Some(if minimum {
        x * -s + y * c
    } else {
        x * c + y * s
    })
}
impl<'a> Surface<'a> {
    fn new(
        shape: &'a Shape,
        mesh: &'a Mesh,
        minimum: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Self, String> {
        let directions = shape
            .tensors
            .iter()
            .zip(&shape.frames)
            .map(|(s, f)| principal(*s, *f, minimum))
            .collect();
        let mut adjacent = vec![[None; 3]; mesh.triangles.len()];
        let mut edges = BTreeMap::<(usize, usize), Vec<(usize, usize)>>::new();
        let mut normals = Vec::with_capacity(mesh.triangles.len());
        for (i, t) in mesh.triangles.iter().enumerate() {
            if i % 128 == 0 && cancel() {
                return Err("Cálculo cancelado.".into());
            }
            let [a, b, c] = t.map(|j| shape.points[j]);
            normals.push((b - a).cross(c - a).normalized());
            for j in 0..3 {
                let (a, b) = (t[(j + 1) % 3], t[(j + 2) % 3]);
                edges.entry((a.min(b), a.max(b))).or_default().push((i, j));
            }
        }
        for faces in edges.values() {
            if let [(a, i), (b, j)] = faces.as_slice() {
                if mesh.face_objects.get(*a) != mesh.face_objects.get(*b) {
                    continue;
                }
                // An unoriented normal test also handles imported winding.
                if normals[*a].dot(normals[*b]).abs() < 0.25 {
                    continue;
                }
                adjacent[*a][*i] = Some(*b);
                adjacent[*b][*j] = Some(*a);
            }
        }
        let mut components = vec![usize::MAX; adjacent.len()];
        for start in 0..adjacent.len() {
            if components[start] != usize::MAX {
                continue;
            }
            let mut stack = vec![start];
            components[start] = start;
            while let Some(i) = stack.pop() {
                for j in adjacent[i].iter().flatten() {
                    if components[*j] == usize::MAX {
                        components[*j] = start;
                        stack.push(*j);
                    }
                }
            }
        }
        Ok(Self {
            shape,
            mesh,
            directions,
            normals,
            adjacent,
            components,
        })
    }
    fn bary(&self, face: usize, q: V3) -> [f64; 3] {
        let [a, b, c] = self.mesh.triangles[face].map(|i| self.shape.points[i]);
        let (e, f, v) = (b - a, c - a, q - a);
        let (ee, ef, ff) = (e.dot(e), e.dot(f), f.dot(f));
        let det = (ee * ff - ef * ef).max(1e-30);
        let u = (ff * v.dot(e) - ef * v.dot(f)) / det;
        let v = (ee * v.dot(f) - ef * v.dot(e)) / det;
        [1. - u - v, u, v]
    }
    fn direction(&self, face: usize, p: V3, previous: Option<V3>) -> Option<V3> {
        let t = self.mesh.triangles[face];
        let reference = previous.or(self.directions[t[0]])?;
        let bary = self.bary(face, p);
        let mut dir = V3::default();
        for j in 0..3 {
            let mut v = self.directions[t[j]]?;
            if v.dot(reference) < 0. {
                v = v * -1.;
            }
            dir = dir + v * bary[j];
        }
        dir = (dir - self.normals[face] * dir.dot(self.normals[face])).normalized();
        (dir.length() > 0.5 && dir.dot(reference.normalized()) > 0.25).then_some(dir)
    }
    fn walk(
        &self,
        mut face: usize,
        mut p: V3,
        mut previous: V3,
        mut remaining: f64,
    ) -> (Vec<(usize, V3, V3)>, bool) {
        let mut points = vec![];
        for _ in 0..64 {
            let Some(mut dir) = self.direction(face, p, Some(previous)) else {
                return (points, false);
            };
            let mid = p + dir * (remaining * 0.5);
            if self.bary(face, mid).iter().all(|v| *v >= 0.) {
                if let Some(v) = self.direction(face, mid, Some(dir)) {
                    dir = v;
                }
            }
            let bary = self.bary(face, p);
            let ahead = self.bary(face, p + dir);
            let mut distance = remaining;
            let mut exit = None;
            for j in 0..3 {
                let delta = ahead[j] - bary[j];
                if delta < -1e-12 {
                    let t = bary[j].max(0.) / -delta;
                    if t < distance {
                        distance = t;
                        exit = Some(j);
                    }
                }
            }
            if distance < self.shape.size * 1e-12 {
                return (points, false);
            }
            p = p + dir * distance;
            remaining -= distance;
            points.push((face, p, dir));
            if remaining < self.shape.size * 1e-10 {
                return (points, true);
            }
            let Some(next) = exit.and_then(|edge| self.adjacent[face][edge]) else {
                return (points, false);
            };
            face = next;
            previous = dir;
        }
        (points, false)
    }
}
struct Occupancy {
    size: f64,
    cells: HashMap<(usize, i64, i64, i64), Vec<V3>>,
}
impl Occupancy {
    fn key(&self, component: usize, p: V3) -> (usize, i64, i64, i64) {
        (
            component,
            (p.x / self.size).floor() as i64,
            (p.y / self.size).floor() as i64,
            (p.z / self.size).floor() as i64,
        )
    }
    fn near(&self, component: usize, p: V3) -> bool {
        let (c, x, y, z) = self.key(component, p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if self
                        .cells
                        .get(&(c, x + dx, y + dy, z + dz))
                        .is_some_and(|v| v.iter().any(|q| (*q - p).length() < self.size * 0.72))
                    {
                        return true;
                    }
                }
            }
        }
        false
    }
    fn add(&mut self, component: usize, p: V3) {
        self.cells
            .entry(self.key(component, p))
            .or_default()
            .push(p);
    }
}
fn trace(
    surface: &Surface,
    face: usize,
    seed: V3,
    sign: f64,
    step: f64,
    length: f64,
    occupied: &Occupancy,
    cancel: &dyn Fn() -> bool,
) -> Result<(Vec<V3>, bool), String> {
    let Some(dir) = surface.direction(face, seed, None) else {
        return Ok((vec![], false));
    };
    let component = surface.components[face];
    let (mut face, mut p, mut dir, mut left) = (face, seed, dir * sign, length);
    let mut out = vec![];
    for _ in 0..4096 {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let (points, complete) = surface.walk(face, p, dir, step.min(left));
        for (next, q, direction) in points {
            if occupied.near(component, q) {
                return Ok((out, false));
            }
            left -= (q - p).length();
            p = q;
            face = next;
            dir = direction;
            if out.len() > 8 && (q - seed).length() < step * 0.75 {
                out.push(seed);
                return Ok((out, true));
            }
            out.push(q);
            if left <= step * 1e-6 {
                return Ok((out, false));
            }
        }
        if !complete {
            break;
        }
    }
    Ok((out, false))
}
pub(super) fn draw(
    shape: &Shape,
    mesh: &Mesh,
    minimum: bool,
    spacing: f64,
    length: f64,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Vec<V3>>, String> {
    if mesh.triangles.is_empty() {
        return Ok(vec![]);
    }
    let surface = Surface::new(shape, mesh, minimum, cancel)?;
    let mut cumulative = Vec::with_capacity(mesh.triangles.len());
    let mut area = 0.;
    for t in &mesh.triangles {
        let [a, b, c] = t.map(|i| shape.points[i]);
        area += (b - a).cross(c - a).length() * 0.5;
        cumulative.push(area);
    }
    let candidates = (area / (spacing * spacing) * 3.).ceil().min(60_000.) as usize;
    let mut occupied = Occupancy {
        size: spacing,
        cells: HashMap::new(),
    };
    let step = spacing * 0.22;
    let mut lines = vec![];
    let mut points = 0;
    for i in 0..candidates {
        if i % 32 == 0 && cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let target = (i as f64 + 0.5) / candidates as f64 * area;
        let face = cumulative
            .partition_point(|v| *v < target)
            .min(mesh.triangles.len() - 1);
        let u = ((i as f64 * 0.7548776662466927 + 0.37).fract()).sqrt();
        let v = (i as f64 * 0.5698402909980532 + 0.19).fract();
        let [a, b, c] = mesh.triangles[face].map(|i| shape.points[i]);
        let seed = a * (1. - u) + b * (u * (1. - v)) + c * (u * v);
        let component = surface.components[face];
        if occupied.near(component, seed) {
            continue;
        }
        let (forward, closed) = trace(
            &surface,
            face,
            seed,
            1.,
            step,
            length * 0.5,
            &occupied,
            cancel,
        )?;
        let mut line = if closed {
            vec![seed]
        } else {
            let (mut backward, _) = trace(
                &surface,
                face,
                seed,
                -1.,
                step,
                length * 0.5,
                &occupied,
                cancel,
            )?;
            backward.reverse();
            backward.push(seed);
            backward
        };
        line.extend(forward);
        let total = line.windows(2).map(|p| (p[1] - p[0]).length()).sum::<f64>();
        if total < spacing.min(length * 0.3) {
            continue;
        }
        points += line.len();
        if points > 250_000 {
            return Err("Demasiados puntos de curvatura; aumenta la separación.".into());
        }
        for q in &line {
            occupied.add(component, *q);
        }
        lines.push(line);
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn shape(mesh: &Mesh) -> Shape {
        let g = fields::Cache::default()
            .geometry(Arc::new(mesh.clone()), &|| false)
            .unwrap();
        Shape::new(mesh, &g.normals, 0, &|| false).unwrap()
    }
    fn cylinder() -> (Mesh, Shape) {
        let (nu, nv) = (48, 13);
        let mut vertices = vec![];
        let mut triangles = vec![];
        for z in 0..nv {
            for i in 0..nu {
                let a = i as f64 / (nu - 1) as f64 * std::f64::consts::PI * 1.5;
                vertices.push([a.cos(), a.sin(), z as f64 / (nv - 1) as f64 * 2. - 1.]);
            }
        }
        for z in 0..nv - 1 {
            for i in 0..nu - 1 {
                let a = z * nu + i;
                triangles.extend([[a, a + 1, a + nu + 1], [a, a + nu + 1, a + nu]]);
            }
        }
        let mesh = Mesh {
            name: "Cylinder patch".into(),
            vertices,
            triangles,
            objects: vec![],
            face_objects: vec![],
        };
        let mut s = shape(&mesh);
        // Exact cylinder tensor, isolating integration from fitting accuracy.
        for i in 0..s.points.len() {
            let p = s.points[i];
            let n = V3::new(p.x, p.y, 0.).normalized();
            let tangent = V3::new(-p.y, p.x, 0.).normalized();
            s.normals[i] = n;
            s.frames[i] = frame(n);
            let [x, y] = s.frames[i];
            let (u, v) = (tangent.dot(x), tangent.dot(y));
            s.tensors[i] = [u * u, u * v, v * v];
        }
        (mesh, s)
    }
    #[test]
    fn eigenvectors_are_orthogonal_and_umbilics_have_no_arbitrary_direction() {
        let basis = [V3::new(1., 0., 0.), V3::new(0., 1., 0.)];
        let a = principal([2., 1., 2.], basis, false).unwrap();
        let b = principal([2., 1., 2.], basis, true).unwrap();
        assert!(a.dot(b).abs() < 1e-12 && (a.x - a.y).abs() < 1e-12);
        assert!(principal([1., 0., 1.], basis, false).is_none());
        assert!(principal([0.; 3], basis, true).is_none());
    }
    #[test]
    fn cylinder_families_follow_circumference_or_axis_and_stay_on_triangles() {
        let (mesh, s) = cylinder();
        for minimum in [false, true] {
            let lines = draw(&s, &mesh, minimum, 0.16, 0.8, &|| false).unwrap();
            assert!(!lines.is_empty());
            let surface = Surface::new(&s, &mesh, minimum, &|| false).unwrap();
            for line in &lines {
                let length = line.windows(2).map(|w| (w[1] - w[0]).length()).sum::<f64>();
                assert!(length <= 0.8 + 1e-8);
                for p in line {
                    assert!(p.finite() && p.z.abs() <= 1. + 1e-8);
                    if minimum {
                        assert!((p.x - line[0].x).abs() < 1e-8 && (p.y - line[0].y).abs() < 1e-8);
                    } else {
                        assert!((p.z - line[0].z).abs() < 1e-8);
                    }
                }
                for pair in line.windows(2).take(8) {
                    let mid = (pair[0] + pair[1]) * 0.5;
                    assert!(mesh.triangles.iter().enumerate().any(|(i, t)| {
                        (mid - s.points[t[0]]).dot(surface.normals[i]).abs() < 1e-9
                            && surface.bary(i, mid).iter().all(|b| *b >= -1e-8)
                    }));
                }
            }
        }
    }
    #[test]
    fn density_is_deterministic_and_cancellation_keeps_the_input() {
        let (mesh, s) = cylinder();
        let original = mesh.clone();
        let dense = draw(&s, &mesh, false, 0.12, 0.8, &|| false).unwrap();
        assert_eq!(dense, draw(&s, &mesh, false, 0.12, 0.8, &|| false).unwrap());
        let sparse = draw(&s, &mesh, false, 0.3, 0.8, &|| false).unwrap();
        let total = |lines: &Vec<Vec<V3>>| {
            lines
                .iter()
                .flat_map(|l| l.windows(2))
                .map(|w| (w[1] - w[0]).length())
                .sum::<f64>()
        };
        assert!(total(&dense) > total(&sparse));
        assert!(draw(&s, &mesh, false, 0.12, 0.8, &|| true).is_err());
        assert_eq!(mesh, original);
    }
    #[test]
    fn occupancy_keeps_close_disconnected_surfaces_independent() {
        let (mesh, mut s) = cylinder();
        let mut copy = mesh.clone();
        let count = copy.vertices.len();
        copy.vertices
            .extend(mesh.vertices.iter().map(|p| [p[0], p[1], p[2] + 0.02]));
        copy.triangles
            .extend(mesh.triangles.iter().map(|t| t.map(|i| i + count)));
        let mut both = shape(&copy);
        for i in 0..count {
            both.tensors[i] = s.tensors[i];
            both.tensors[i + count] = s.tensors[i];
            both.frames[i] = s.frames[i];
            both.frames[i + count] = s.frames[i];
        }
        let surface = Surface::new(&both, &copy, false, &|| false).unwrap();
        assert_ne!(
            surface.components[0],
            surface.components[mesh.triangles.len()]
        );
        let mut occupancy = Occupancy {
            size: 0.2,
            cells: HashMap::new(),
        };
        let p = s.points[0];
        occupancy.add(surface.components[0], p);
        assert!(occupancy.near(surface.components[0], p));
        assert!(!occupancy.near(
            surface.components[mesh.triangles.len()],
            p + V3::new(0., 0., 0.02)
        ));
        s.tensors.fill([1., 0., 1.]);
        assert!(draw(&s, &mesh, false, 0.2, 0.8, &|| false)
            .unwrap()
            .is_empty());
    }
}
