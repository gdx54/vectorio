//! View-dependent feature lines on a piecewise smooth surface. The fitted
//! symmetric normal derivative is intrinsic; only the view field is recomputed
//! when orbiting. See docs/volumen/contours-and-large-models.md for definitions/sources.
use super::*;
use std::collections::BTreeMap;
#[path = "flow.rs"]
mod flow;

struct Shape {
    points: Vec<V3>,
    normals: Vec<V3>,
    frames: Vec<[V3; 2]>,
    tensors: Vec<[f64; 3]>, // [[a,b],[b,c]] = Dn
    neighbors: Vec<Vec<usize>>,
    boundary: Vec<bool>,
    size: f64,
}
fn frame(n: V3) -> [V3; 2] {
    let axis = if n.z.abs() < 0.8 {
        V3::new(0., 0., 1.)
    } else {
        V3::new(1., 0., 0.)
    };
    let x = axis.cross(n).normalized();
    [x, n.cross(x).normalized()]
}
fn solve(mut m: [[f64; 3]; 3], mut b: [f64; 3]) -> [f64; 3] {
    let regularizer = (m[0][0] + m[1][1] + m[2][2]) * 1e-10 + 1e-20;
    for i in 0..3 {
        m[i][i] += regularizer;
    }
    for i in 0..3 {
        let pivot = (i..3)
            .max_by(|a, b| m[*a][i].abs().total_cmp(&m[*b][i].abs()))
            .unwrap();
        m.swap(i, pivot);
        b.swap(i, pivot);
        if m[i][i].abs() < 1e-25 {
            return [0.; 3];
        }
        for j in i + 1..3 {
            let f = m[j][i] / m[i][i];
            for k in i..3 {
                m[j][k] -= f * m[i][k];
            }
            b[j] -= f * b[i];
        }
    }
    let mut x = [0.; 3];
    for i in (0..3).rev() {
        x[i] = (b[i] - (i + 1..3).map(|j| m[i][j] * x[j]).sum::<f64>()) / m[i][i];
    }
    x
}
impl Shape {
    fn new(
        mesh: &Mesh,
        normals: &[V3],
        smoothing: usize,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Self, String> {
        let points = mesh.points();
        let mut neighbors = vec![vec![]; points.len()];
        let mut edges = BTreeMap::<(usize, usize), usize>::new();
        for (i, t) in mesh.triangles.iter().enumerate() {
            if i % 128 == 0 && cancel() {
                return Err("Cálculo cancelado.".into());
            }
            for j in 0..3 {
                let (a, b) = (t[j], t[(j + 1) % 3]);
                neighbors[a].push(b);
                neighbors[b].push(a);
                *edges.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let mut boundary = vec![false; points.len()];
        for ((a, b), count) in edges {
            if count != 2 {
                boundary[a] = true;
                boundary[b] = true;
            }
        }
        for n in &mut neighbors {
            n.sort_unstable();
            n.dedup();
        }
        let mut normals = normals.to_vec();
        for _ in 0..smoothing {
            if cancel() {
                return Err("Cálculo cancelado.".into());
            }
            normals = neighbors
                .iter()
                .enumerate()
                .map(|(i, ids)| {
                    if ids.is_empty() {
                        return normals[i];
                    }
                    let sum = ids.iter().fold(V3::default(), |n, j| n + normals[*j]);
                    (normals[i] + sum * (1. / ids.len() as f64)).normalized()
                })
                .collect();
        }
        let frames: Vec<_> = normals.iter().copied().map(frame).collect();
        let mut tensors = Vec::with_capacity(points.len());
        for (i, ids) in neighbors.iter().enumerate() {
            if i % 128 == 0 && cancel() {
                return Err("Cálculo cancelado.".into());
            }
            let [tx, ty] = frames[i];
            let mut m = [[0.; 3]; 3];
            let mut rhs = [0.; 3];
            for &j in ids {
                let edge = points[j] - points[i];
                let dn = normals[j] - normals[i];
                let (x, y, u, v) = (edge.dot(tx), edge.dot(ty), dn.dot(tx), dn.dot(ty));
                let w = 1. / (x * x + y * y).max(1e-20);
                m[0][0] += w * x * x;
                m[0][1] += w * x * y;
                m[1][1] += w * (x * x + y * y);
                m[1][2] += w * x * y;
                m[2][2] += w * y * y;
                rhs[0] += w * x * u;
                rhs[1] += w * (y * u + x * v);
                rhs[2] += w * y * v;
            }
            m[1][0] = m[0][1];
            m[2][1] = m[1][2];
            tensors.push(solve(m, rhs));
        }
        let mut lo = V3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut hi = lo * -1.;
        for p in &points {
            lo = V3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = V3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
        Ok(Self {
            points,
            normals,
            frames,
            tensors,
            neighbors,
            boundary,
            size: (hi - lo).length().max(1e-12),
        })
    }
    fn gradient(
        &self,
        values: &[f64],
        valid: Option<&[bool]>,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Vec<V3>, String> {
        let mut out = Vec::with_capacity(values.len());
        for (i, ids) in self.neighbors.iter().enumerate() {
            if i % 128 == 0 && cancel() {
                return Err("Cálculo cancelado.".into());
            }
            let [tx, ty] = self.frames[i];
            let (mut xx, mut xy, mut yy, mut xz, mut yz) = (0., 0., 0., 0., 0.);
            for &j in ids {
                if valid.is_some_and(|v| !v[j]) {
                    continue;
                }
                let e = self.points[j] - self.points[i];
                let (x, y, z) = (e.dot(tx), e.dot(ty), values[j] - values[i]);
                let w = 1. / (x * x + y * y).max(1e-20);
                xx += w * x * x;
                xy += w * x * y;
                yy += w * y * y;
                xz += w * x * z;
                yz += w * y * z;
            }
            let det = xx * yy - xy * xy;
            out.push(if det.abs() > 1e-16 {
                tx * ((yy * xz - xy * yz) / det) + ty * ((xx * yz - xy * xz) / det)
            } else {
                V3::default()
            });
        }
        Ok(out)
    }
}
#[derive(Default)]
pub(super) struct Cache {
    entries: Vec<(Arc<Mesh>, usize, Arc<Shape>)>,
    flows: Vec<(Arc<Shape>, bool, f64, f64, Arc<Vec<Vec<V3>>>)>,
    #[cfg(test)]
    pub builds: usize,
    #[cfg(test)]
    pub flow_builds: usize,
}
impl Cache {
    fn shape(
        &mut self,
        mesh: Arc<Mesh>,
        normals: &[V3],
        smoothing: usize,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Arc<Shape>, String> {
        if let Some((_, _, s)) = self
            .entries
            .iter()
            .find(|(m, n, _)| Arc::ptr_eq(m, &mesh) && *n == smoothing)
        {
            return Ok(s.clone());
        }
        let shape = Arc::new(Shape::new(&mesh, normals, smoothing, cancel)?);
        if self.entries.len() >= 3 {
            self.entries.remove(0);
        }
        self.entries.push((mesh, smoothing, shape.clone()));
        #[cfg(test)]
        {
            self.builds += 1;
        }
        Ok(shape)
    }
    pub fn draw(
        &mut self,
        d: &Drawing,
        s: &Settings,
        normals: &[V3],
        effect: &ReadingEffect,
        paper: &crate::Paper,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Vec<Polyline>, String> {
        let smoothing = match *effect {
            ReadingEffect::Suggestive { smoothing, .. }
            | ReadingEffect::ApparentRidges { smoothing, .. }
            | ReadingEffect::PrincipalFlow { smoothing, .. } => smoothing,
            _ => unreachable!(),
        };
        let shape = self.shape(d.mesh.clone(), normals, smoothing, cancel)?;
        if let ReadingEffect::PrincipalFlow {
            minimum,
            spacing_mm,
            length_mm,
            ..
        } = *effect
        {
            let spacing = spacing_mm / d.camera.scale;
            let length = length_mm / d.camera.scale;
            let curves = if let Some((_, _, _, _, curves)) =
                self.flows.iter().find(|(s, m, sp, l, _)| {
                    Arc::ptr_eq(s, &shape) && *m == minimum && *sp == spacing && *l == length
                }) {
                curves.clone()
            } else {
                let curves = Arc::new(flow::draw(
                    &shape, &d.mesh, minimum, spacing, length, cancel,
                )?);
                if self.flows.len() >= 3 {
                    self.flows.remove(0);
                }
                self.flows
                    .push((shape, minimum, spacing, length, curves.clone()));
                #[cfg(test)]
                {
                    self.flow_builds += 1;
                }
                curves
            };
            let segments = curves
                .iter()
                .flat_map(|line| line.windows(2).map(|p| [p[0], p[1]]))
                .collect::<Vec<_>>();
            return visible(&segments, d, s, paper, cancel);
        }
        let segments = extract(&shape, &d.mesh, d.camera, effect, cancel)?;
        visible(&segments, d, s, paper, cancel)
    }
}
// q1 and t1 of Q = Dn P^-1, where P maps the tangent plane to a
// local viewing plane. t1 is lifted back to the surface before differentiation.
fn apparent([a, b, c]: [f64; 3], [tx, ty]: [V3; 2], view: V3, right: V3) -> (f64, V3, bool) {
    let sx = (right - view * right.dot(view)).normalized();
    let sy = sx.cross(view);
    let (p00, p01, p10, p11) = (tx.dot(sx), ty.dot(sx), tx.dot(sy), ty.dot(sy));
    let det = p00 * p11 - p01 * p10;
    if det.abs() < 0.02 {
        return (0., V3::default(), false);
    }
    let inv = [p11 / det, -p01 / det, -p10 / det, p00 / det];
    let q = [
        a * inv[0] + b * inv[2],
        a * inv[1] + b * inv[3],
        b * inv[0] + c * inv[2],
        b * inv[1] + c * inv[3],
    ];
    let (xx, xy, yy) = (
        q[0] * q[0] + q[2] * q[2],
        q[0] * q[1] + q[2] * q[3],
        q[1] * q[1] + q[3] * q[3],
    );
    let gap = ((xx - yy) * (xx - yy) + 4. * xy * xy).sqrt();
    let lambda = (xx + yy + gap) * 0.5;
    let angle = 0.5 * (2. * xy).atan2(xx - yy);
    let (ey, ex) = angle.sin_cos();
    let dir = tx * (inv[0] * ex + inv[1] * ey) + ty * (inv[2] * ex + inv[3] * ey);
    (
        lambda.max(0.).sqrt(),
        dir,
        gap > lambda * 1e-4 && lambda > 1e-20,
    )
}
fn extract(
    shape: &Shape,
    mesh: &Mesh,
    camera: Camera,
    effect: &ReadingEffect,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<[V3; 2]>, String> {
    let apparent_mode = matches!(effect, ReadingEffect::ApparentRidges { .. });
    let threshold = match *effect {
        ReadingEffect::Suggestive { threshold, .. }
        | ReadingEffect::ApparentRidges { threshold, .. } => threshold,
        _ => unreachable!(),
    };
    let mut scalar = Vec::new();
    let mut directions = Vec::new();
    let mut facing = Vec::new();
    let mut valid = Vec::new();
    let (right, _, away) = camera.axes();
    let eye = camera
        .perspective
        .map(|[distance, _]| camera.world_center - away * distance);
    for i in 0..shape.points.len() {
        if i % 128 == 0 && cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let view = eye.map_or(away * -1., |eye| (eye - shape.points[i]).normalized());
        let n = shape.normals[i];
        let nv = n.dot(view);
        let [tx, ty] = shape.frames[i];
        let [a, b, c] = shape.tensors[i];
        let (value, dir, ok) = if apparent_mode {
            apparent([a, b, c], [tx, ty], view, right)
        } else {
            let r = (view - n * nv).normalized();
            let (x, y) = (r.dot(tx), r.dot(ty));
            (a * x * x + 2. * b * x * y + c * y * y, r, r.length() > 0.5)
        };
        scalar.push(value);
        directions.push(dir);
        facing.push(nv);
        valid.push(ok && !shape.boundary[i] && nv > 0.02);
    }
    // Q becomes singular at grazing incidence. Its rejected samples are not
    // zeros of q1: including them in a gradient invents ridges at the silhouette.
    let gradient = shape.gradient(&scalar, apparent_mode.then_some(valid.as_slice()), cancel)?;
    if apparent_mode {
        let supported: Vec<_> = shape
            .neighbors
            .iter()
            .map(|ids| ids.iter().filter(|j| valid[**j]).count() >= 3)
            .collect();
        for (v, supported) in valid.iter_mut().zip(supported) {
            *v &= supported;
        }
    }
    let derivative: Vec<_> = gradient
        .iter()
        .zip(&directions)
        .map(|(g, r)| g.dot(*r))
        .collect();
    let mut segments = Vec::new();
    for (index, t) in mesh.triangles.iter().enumerate() {
        if index % 128 == 0 && cancel() {
            return Err("Cálculo cancelado.".into());
        }
        if !t.iter().all(|i| valid[*i]) {
            continue;
        }
        let points = t.map(|i| shape.points[i]);
        let mut values = t.map(|i| {
            if apparent_mode {
                derivative[i]
            } else {
                scalar[i]
            }
        });
        let gates = t.map(|i| {
            if apparent_mode {
                scalar[i] * shape.size - threshold
            } else {
                derivative[i] * shape.size * shape.size - threshold
            }
        });
        if apparent_mode {
            let dir = directions[t[0]];
            for j in 1..3 {
                if dir.dot(directions[t[j]]) < 0. {
                    values[j] *= -1.;
                }
            }
            // Maxima only: the aligned directional derivative must decrease
            // along t1. This also rejects flat/constant fields and minima.
            let e = points[1] - points[0];
            let f = points[2] - points[0];
            let n = e.cross(f);
            let grad = (f.cross(n) * (values[1] - values[0])
                + n.cross(e) * (values[2] - values[0]))
                * (1. / n.dot(n).max(1e-30));
            let along = t.iter().fold(V3::default(), |sum, i| {
                sum + directions[*i]
                    * if dir.dot(directions[*i]) < 0. {
                        -1.
                    } else {
                        1.
                    }
            });
            if grad.dot(along) >= -1e-12 / (shape.size * shape.size * shape.size) {
                continue;
            }
        } else if let ReadingEffect::Suggestive { min_angle_deg, .. } = effect {
            let max_nv = min_angle_deg.to_radians().cos();
            // A second gate is clipped separately below (unlike testing only
            // the triangle midpoint, this keeps valid partial segments).
            if t.iter().all(|i| facing[*i] >= max_nv) {
                continue;
            }
        }
        let mut cuts: Vec<(V3, [f64; 3])> = Vec::new();
        for j in 0..3 {
            let k = (j + 1) % 3;
            let (a, b) = (values[j], values[k]);
            if (a < 0.) == (b < 0.) || (a - b).abs() < 1e-16 / shape.size {
                continue;
            }
            let u = a / (a - b);
            let mut bary = [0.; 3];
            bary[j] = 1. - u;
            bary[k] = u;
            cuts.push((points[j] + (points[k] - points[j]) * u, bary));
        }
        if cuts.len() != 2 || (cuts[0].0 - cuts[1].0).length() < shape.size * 1e-12 {
            continue;
        }
        let mut range = [0., 1.];
        let mut clip = |g: [f64; 3]| {
            let value = |b: [f64; 3]| (0..3).map(|j| b[j] * g[j]).sum::<f64>();
            let (a, b) = (value(cuts[0].1), value(cuts[1].1));
            if a < 0. && b < 0. {
                return false;
            }
            if a < 0. {
                range[0] = f64::max(range[0], a / (a - b));
            }
            if b < 0. {
                range[1] = f64::min(range[1], a / (a - b));
            }
            range[1] > range[0]
        };
        if !clip(gates) {
            continue;
        }
        if let ReadingEffect::Suggestive { min_angle_deg, .. } = effect {
            if !clip(t.map(|i| min_angle_deg.to_radians().cos() - facing[i])) {
                continue;
            }
        }
        let a = cuts[0].0;
        let e = cuts[1].0 - a;
        segments.push([a + e * range[0], a + e * range[1]]);
    }
    Ok(segments)
}
fn visible(
    segments: &[[V3; 2]],
    d: &Drawing,
    s: &Settings,
    paper: &crate::Paper,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Polyline>, String> {
    if segments.is_empty() {
        return Ok(vec![]);
    }
    let camera = d.camera;
    let mut render = camera;
    render.scale /= 1000.;
    render.world_center = render.world_center * 1000.;
    render.perspective = render.perspective.map(|[d, n]| [d * 1000., n * 1000.]);
    let mut scene = crate::scene::Scene::new(render, effects::area(paper));
    scene.set_intersections(false);
    let rotate = |p: V3| {
        let q = p - camera.world_center;
        let axis = camera.axes().2;
        let (s, c) = s.paper_rotation.to_radians().sin_cos();
        (camera.world_center + q * c + axis.cross(q) * s + axis * (axis.dot(q) * (1. - c))) * 1000.
    };
    let points = d.mesh.points();
    for (i, t) in d.mesh.triangles.iter().enumerate() {
        if i % 128 == 0 && cancel() {
            return Err("Cálculo cancelado.".into());
        }
        scene.occluder(&t.map(|i| rotate(points[i])));
    }
    for line in segments {
        scene.line(&line.map(rotate));
    }
    let drawing = scene
        .render_cancellable(cancel)
        .ok_or("Cálculo cancelado.")?;
    let edges = drawing
        .parts
        .into_iter()
        .flat_map(|p| p.lines)
        .flat_map(|l| l.windows(2).map(|p| [p[0], p[1]]).collect::<Vec<_>>())
        .collect();
    scalar::join(edges, cancel)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn shape(mesh: &Mesh) -> Shape {
        let geometry = fields::Cache::default()
            .geometry(Arc::new(mesh.clone()), &|| false)
            .unwrap();
        Shape::new(mesh, &geometry.normals, 0, &|| false).unwrap()
    }
    fn effects() -> [ReadingEffect; 2] {
        [
            ReadingEffect::Suggestive {
                threshold: 0.1,
                min_angle_deg: 20.,
                smoothing: 0,
            },
            ReadingEffect::ApparentRidges {
                threshold: 2.,
                smoothing: 0,
            },
        ]
    }
    fn camera() -> Camera {
        Camera {
            yaw_deg: 31.,
            elev_deg: 28.,
            ..Default::default()
        }
    }
    fn grid(f: impl Fn(f64, f64) -> f64) -> Mesh {
        let n = 35;
        let mut vertices = vec![];
        let mut triangles = vec![];
        for y in 0..n {
            for x in 0..n {
                let (x, y) = (
                    x as f64 / (n - 1) as f64 * 4. - 2.,
                    y as f64 / (n - 1) as f64 * 4. - 2.,
                );
                vertices.push([x, y, f(x, y)]);
            }
        }
        for y in 0..n - 1 {
            for x in 0..n - 1 {
                let i = y * n + x;
                triangles.extend([[i, i + 1, i + n + 1], [i, i + n + 1, i + n]]);
            }
        }
        Mesh {
            name: "Test surface".into(),
            vertices,
            triangles,
            objects: vec![],
            face_objects: vec![],
        }
    }
    #[test]
    fn symmetric_tensor_recovers_sphere_curvature_and_inverse_scale() {
        let mesh = Primitive::Sphere.mesh();
        let s = shape(&mesh);
        let mean =
            s.tensors.iter().map(|[a, _, c]| (a + c) * 0.5).sum::<f64>() / s.tensors.len() as f64;
        assert!((mean - 1.).abs() < 0.06, "{mean}");
        let mut large = mesh.clone();
        for p in &mut large.vertices {
            for v in p {
                *v *= 7.;
            }
        }
        let scaled = shape(&large);
        for (a, b) in s.tensors.iter().zip(&scaled.tensors) {
            for i in 0..3 {
                assert!((a[i] - 7. * b[i]).abs() < 1e-7);
            }
        }
        assert!(extract(&s, &mesh, camera(), &effects()[0], &|| false)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn planes_have_no_feature_lines_and_constant_apparent_curvature_has_no_ridge() {
        let mesh = grid(|x, y| 0.2 * x + 0.1 * y);
        let s = shape(&mesh);
        for e in effects() {
            assert!(extract(&s, &mesh, camera(), &e, &|| false)
                .unwrap()
                .is_empty());
        }
        let (q, dir, valid) = apparent(
            [2., 0., 1.],
            [V3::new(1., 0., 0.), V3::new(0., 1., 0.)],
            V3::new(0., 0., 1.),
            V3::new(1., 0., 0.),
        );
        assert!(valid && (q - 2.).abs() < 1e-10 && dir.x.abs() > 0.99);
    }
    #[test]
    fn torus_features_are_view_dependent_deterministic_and_scale_invariant() {
        let mesh = Primitive::Torus.mesh();
        let s = shape(&mesh);
        for e in effects() {
            let a = extract(&s, &mesh, camera(), &e, &|| false).unwrap();
            assert!(!a.is_empty(), "{e:?}");
            assert_eq!(a, extract(&s, &mesh, camera(), &e, &|| false).unwrap());
            let mut c = camera();
            c.yaw_deg += 27.;
            c.elev_deg += 15.;
            assert_ne!(a, extract(&s, &mesh, c, &e, &|| false).unwrap());
            let mut large = mesh.clone();
            for p in &mut large.vertices {
                for v in p {
                    *v *= 5.;
                }
            }
            let b = extract(&shape(&large), &large, camera(), &e, &|| false).unwrap();
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                for j in 0..2 {
                    assert!((a[j] * 5. - b[j]).length() < 1e-5);
                }
            }
        }
    }
    #[test]
    fn higher_threshold_only_removes_or_trims_suggestive_segments() {
        let mesh = Primitive::Torus.mesh();
        let s = shape(&mesh);
        let length = |e| {
            extract(&s, &mesh, camera(), &e, &|| false)
                .unwrap()
                .iter()
                .map(|[a, b]| (*b - *a).length())
                .sum::<f64>()
        };
        let mut last = f64::INFINITY;
        for threshold in [0., 0.1, 1., 5., 20.] {
            let n = length(ReadingEffect::Suggestive {
                threshold,
                min_angle_deg: 20.,
                smoothing: 0,
            });
            assert!(n <= last + 1e-8);
            last = n;
        }
    }
    #[test]
    fn apparent_ridges_follow_the_top_of_a_smooth_cylindrical_bump() {
        let mesh = grid(|x, _| (-x * x * 2.).exp());
        let s = shape(&mesh);
        let c = Camera {
            yaw_deg: 0.,
            elev_deg: 89.999,
            ..Default::default()
        };
        let lines = extract(
            &s,
            &mesh,
            c,
            &ReadingEffect::ApparentRidges {
                threshold: 2.,
                smoothing: 0,
            },
            &|| false,
        )
        .unwrap();
        assert!(!lines.is_empty());
        assert!(lines
            .iter()
            .any(|[a, b]| a.x.abs() < 0.15 && b.x.abs() < 0.15 && a.z > 0.9 && b.z > 0.9));
    }
    #[test]
    fn apparent_extrema_reject_minima_and_keep_maxima_of_the_view_dependent_field() {
        let mesh = grid(|_, _| 0.);
        let mut s = shape(&mesh);
        let effect = ReadingEffect::ApparentRidges {
            threshold: 0.,
            smoothing: 0,
        };
        // Inject a known tensor field on a plane to isolate the extremum test
        // from normal fitting. The larger eigenvector is world X.
        for maximum in [false, true] {
            for (i, tensor) in s.tensors.iter_mut().enumerate() {
                let x = s.points[i].x;
                *tensor = [0.1, 0., if maximum { 6. - x * x } else { 2. + x * x }];
            }
            let lines = extract(
                &s,
                &mesh,
                Camera {
                    yaw_deg: 0.,
                    elev_deg: 89.999,
                    ..Default::default()
                },
                &effect,
                &|| false,
            )
            .unwrap();
            assert_eq!(!lines.is_empty(), maximum, "maximum={maximum}");
        }
    }
    #[test]
    fn rejected_boundary_samples_do_not_invent_apparent_maxima() {
        let mesh = grid(|_, _| 0.);
        let mut s = shape(&mesh);
        for (i, tensor) in s.tensors.iter_mut().enumerate() {
            *tensor = if s.boundary[i] {
                [0.; 3]
            } else {
                [0.1, 0., 2. + s.points[i].x * 0.5]
            };
        }
        let effect = ReadingEffect::ApparentRidges {
            threshold: 0.,
            smoothing: 0,
        };
        assert!(extract(
            &s,
            &mesh,
            Camera {
                yaw_deg: 0.,
                elev_deg: 89.999,
                ..Default::default()
            },
            &effect,
            &|| false
        )
        .unwrap()
        .is_empty());
    }
    #[test]
    fn cancellation_does_not_cache_partial_shapes() {
        let mesh = Arc::new(Primitive::Torus.mesh());
        let geometry = fields::Cache::default()
            .geometry(mesh.clone(), &|| false)
            .unwrap();
        let mut cache = Cache::default();
        assert!(cache
            .shape(mesh.clone(), &geometry.normals, 1, &|| true)
            .is_err());
        assert!(cache.entries.is_empty());
        let a = cache
            .shape(mesh.clone(), &geometry.normals, 1, &|| false)
            .unwrap();
        let b = cache
            .shape(mesh.clone(), &geometry.normals, 1, &|| false)
            .unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(cache.builds, 1);
        assert!(extract(&a, &mesh, camera(), &effects()[0], &|| true).is_err());
    }
    #[test]
    fn feature_recipes_roundtrip_and_orbit_reuses_intrinsic_shape() {
        for style in [ReadingStyle::Suggestive, ReadingStyle::ApparentRidges] {
            let mut r = Recipe::new(Primitive::Torus.mesh(), Settings::default());
            r.passes = style.passes(&r.settings);
            let mut engine = Engine::default();
            let r = Recipe::from_json(&r.to_json().unwrap()).unwrap();
            let mesh = Arc::new(r.mesh.clone());
            let paper = crate::Paper::default();
            let mut settings = r.settings.clone();
            for perspective in [false, true] {
                settings.projection.perspective = perspective;
                for angle in [0., 24.] {
                    settings.yaw += angle;
                    settings.paper_rotation = 17.;
                    let d = engine
                        .draw_passes(mesh.clone(), &settings, &r.passes, &paper, &|| false)
                        .unwrap();
                    assert!(!d.parts[0].lines.is_empty(), "{style:?}");
                    assert!(d
                        .parts
                        .iter()
                        .flat_map(|p| p.lines.iter().flatten())
                        .all(|p| p.x.is_finite()
                            && p.y.is_finite()
                            && p.x >= paper.margin_mm - 1e-6
                            && p.x <= paper.width_mm - paper.margin_mm + 1e-6
                            && p.y >= paper.margin_mm - 1e-6
                            && p.y <= paper.height_mm - paper.margin_mm + 1e-6));
                }
            }
            assert_eq!(engine.readings.features.builds, 1);
        }
    }
    #[test]
    fn principal_flow_roundtrips_and_reuses_object_space_strokes_when_orbiting() {
        let mesh = Arc::new(Primitive::Torus.mesh());
        let mut scene = Settings::default();
        let paper = crate::Paper::default();
        let mut passes = ReadingStyle::PrincipalFlow.passes(&scene);
        let mut engine = Engine::default();
        let a = engine
            .draw_passes(mesh.clone(), &scene, &passes, &paper, &|| false)
            .unwrap();
        assert!(!a.parts[0].lines.is_empty());
        scene.yaw += 17.;
        let b = engine
            .draw_passes(mesh.clone(), &scene, &passes, &paper, &|| false)
            .unwrap();
        assert_ne!(a.parts[0].lines, b.parts[0].lines);
        assert_eq!(engine.readings.features.flow_builds, 1);
        passes[0].reading.as_mut().unwrap().pens = vec![1];
        engine
            .draw_passes(mesh.clone(), &scene, &passes, &paper, &|| false)
            .unwrap();
        assert_eq!(engine.readings.features.flow_builds, 1);
        if let PassKind::Reading(ReadingParams {
            effect: ReadingEffect::PrincipalFlow { minimum, .. },
            ..
        }) = &mut passes[0].kind
        {
            *minimum = true;
        }
        engine
            .draw_passes(mesh.clone(), &scene, &passes, &paper, &|| false)
            .unwrap();
        assert_eq!(engine.readings.features.flow_builds, 2);
        let mut recipe = Recipe::new(mesh.as_ref().clone(), scene);
        recipe.passes = passes;
        let decoded = Recipe::from_json(&recipe.to_json().unwrap()).unwrap();
        assert_eq!(decoded.passes, recipe.passes);
        recipe.settings.projection.perspective = true;
        recipe.settings.paper_rotation = 23.;
        let d = engine.draw_recipe(&recipe, &|| false).unwrap();
        assert!(!d.parts[0].lines.is_empty());
        assert!(d
            .parts
            .iter()
            .flat_map(|p| p.lines.iter().flatten())
            .all(|p| p.x.is_finite()
                && p.y.is_finite()
                && p.x >= paper.margin_mm - 1e-6
                && p.y >= paper.margin_mm - 1e-6
                && p.x <= paper.width_mm - paper.margin_mm + 1e-6
                && p.y <= paper.height_mm - paper.margin_mm + 1e-6));
        for bad in [f64::NAN, 0., 81.] {
            let p = ReadingParams {
                facets: 6000,
                effect: ReadingEffect::PrincipalFlow {
                    minimum: false,
                    spacing_mm: 2.,
                    length_mm: bad,
                    smoothing: 1,
                },
            };
            assert!(p.validate().is_err());
        }
    }
    #[test]
    fn analytical_visibility_hides_a_feature_behind_an_occluder() {
        let mesh = grid(|_, _| 0.);
        let mut d = Engine::default()
            .draw(
                Arc::new(mesh),
                &Settings {
                    recipe: RecipeKind::XRay,
                    ..Default::default()
                },
                &crate::Paper::default(),
                &|| false,
            )
            .unwrap();
        d.camera = Camera {
            yaw_deg: 0.,
            elev_deg: 89.999,
            scale: 20.,
            paper_center: p(100., 100.),
            ..Default::default()
        };
        let s = Settings::default();
        let paper = crate::Paper::default();
        let front = visible(
            &[[V3::new(-0.5, 0.137, 0.1), V3::new(0.5, 0.137, 0.1)]],
            &d,
            &s,
            &paper,
            &|| false,
        )
        .unwrap();
        let back = visible(
            &[[V3::new(-0.5, 0.137, -0.1), V3::new(0.5, 0.137, -0.1)]],
            &d,
            &s,
            &paper,
            &|| false,
        )
        .unwrap();
        assert!(!front.is_empty());
        assert!(back.is_empty());
    }
}
