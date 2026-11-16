//! Regiones visibles y evaluación baricéntrica; ninguna búsqueda se hace en píxeles.
use super::super::shadow::project;
use super::*;
#[derive(Clone)]
struct Region {
    face: usize,
    poly: Polyline,
    bounds: BBox,
    intensity: Option<f64>,
}
struct Node {
    bounds: BBox,
    start: usize,
    end: usize,
    children: Option<(usize, usize)>,
}
pub(super) struct Surface {
    regions: Vec<Region>,
    ids: Vec<usize>,
    nodes: Vec<Node>,
    triangles: Vec<[P; 3]>,
    world: Vec<[V3; 3]>,
    camera: Camera,
}
#[derive(Clone, Copy)]
pub(super) struct Sample {
    pub point: P,
    pub world: V3,
    pub face: usize,
    pub normal: V3,
    pub value: f64,
}
#[derive(Clone, Copy)]
pub(super) struct Segment {
    pub a: Sample,
    pub b: Sample,
}
impl Surface {
    pub fn new(
        d: &Drawing,
        s: &Settings,
        light: Option<&[ShadedFace]>,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Self, String> {
        let project = |q| {
            let (p, z) = d.camera.project(q);
            (
                d.camera.paper_center
                    + (p - d.camera.paper_center).rotate(s.paper_rotation.to_radians()),
                z,
            )
        };
        let projection = project::Projection::camera(&d.mesh, &d.camera, s.paper_rotation);
        let points = d.mesh.points();
        let world = d
            .mesh
            .triangles
            .iter()
            .map(|t| t.map(|i| points[i]))
            .collect::<Vec<_>>();
        let triangles = world.iter().map(|t| t.map(|p| project(p).0)).collect();
        let mut regions = Vec::new();
        let mut push = |face, poly: project::Polygon, intensity| {
            if poly.len() < 3 {
                return;
            }
            let mut bounds = BBox::empty();
            let poly: Polyline = poly
                .iter()
                .map(|v| {
                    bounds.add(v.point);
                    v.point
                })
                .collect();
            regions.push(Region {
                face,
                poly,
                bounds,
                intensity,
            });
        };
        for (face, _) in d.mesh.triangles.iter().enumerate() {
            if cancel() {
                return Err("Cálculo cancelado.".into());
            }
            let mut visible = projection.visible(face, cancel)?;
            if let Some(light) = light {
                for patch in &light[face].cast {
                    let poly = patch
                        .iter()
                        .map(|q| {
                            let (point, depth) = project(*q);
                            project::Vertex {
                                point,
                                depth,
                                world: *q,
                            }
                        })
                        .collect::<Vec<_>>();
                    let planes = project::boundary(&poly);
                    let mut remainder = Vec::new();
                    for region in &visible {
                        push(face, project::intersection(region, &planes), Some(0.));
                        remainder.extend(project::subtract(region, &planes));
                    }
                    visible = remainder;
                    if visible.len() > 4096 {
                        return Err("Demasiadas regiones de luz; reduce las facetas.".into());
                    }
                }
            }
            for region in visible {
                push(face, region, light.map(|v| v[face].cosine.clamp(0., 1.)));
            }
        }
        if regions.len() > 250_000 {
            return Err("El mapa supera 250.000 regiones; reduce el detalle.".into());
        }
        let ids = (0..regions.len()).collect();
        let mut surface = Self {
            regions,
            ids,
            nodes: vec![],
            triangles,
            world,
            camera: d.camera,
        };
        if !surface.ids.is_empty() {
            surface.build(0, surface.ids.len());
        }
        Ok(surface)
    }
    fn build(&mut self, start: usize, end: usize) -> usize {
        let mut bounds = BBox::empty();
        for &i in &self.ids[start..end] {
            bounds.union(&self.regions[i].bounds);
        }
        let id = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            start,
            end,
            children: None,
        });
        if end - start > 12 {
            let x = bounds.width() >= bounds.height();
            self.ids[start..end].sort_by(|a, b| {
                let a = self.regions[*a].bounds.center();
                let b = self.regions[*b].bounds.center();
                (if x { a.x } else { a.y }).total_cmp(&if x { b.x } else { b.y })
            });
            let mid = (start + end) / 2;
            let a = self.build(start, mid);
            let b = self.build(mid, end);
            self.nodes[id].children = Some((a, b));
        }
        id
    }
    fn candidates(&self, bounds: BBox) -> Vec<usize> {
        if self.nodes.is_empty() {
            return vec![];
        }
        let mut stack = vec![0];
        let mut out = Vec::new();
        while let Some(i) = stack.pop() {
            let node = &self.nodes[i];
            if !overlap(bounds, node.bounds) {
                continue;
            }
            if let Some((a, b)) = node.children {
                stack.extend([a, b]);
            } else {
                out.extend(
                    self.ids[node.start..node.end]
                        .iter()
                        .copied()
                        .filter(|i| overlap(bounds, self.regions[*i].bounds)),
                );
            }
        }
        out
    }
    fn sample(
        &self,
        id: usize,
        point: P,
        values: &fields::Values,
        geometry: &fields::Geometry,
        mesh: &Mesh,
    ) -> Sample {
        let region = &self.regions[id];
        let face = region.face;
        let weights = self
            .camera
            .weights(barycentric(point, self.triangles[face]), self.world[face]);
        let t = mesh.triangles[face];
        let interpolate = |v: &[f64]| (0..3).map(|i| weights[i] * v[t[i]]).sum::<f64>();
        let mut value = interpolate(&values.values);
        if values.config.measure == Measure::Aspect && values.config.secondary.is_none() {
            let base = values.values[t[0]];
            value = (0..3)
                .map(|i| {
                    weights[i]
                        * (base + (values.values[t[i]] - base + 180.).rem_euclid(360.) - 180.)
                })
                .sum();
        }
        if values.config.measure == Measure::Objects {
            value = mesh.face_objects.get(face).copied().unwrap_or(0) as f64;
        }
        if let (Some(intensity), Some(vertex)) = (region.intensity, values.light_vertex.as_ref()) {
            value += (intensity - interpolate(vertex)) * values.light_weight;
        }
        let world = (0..3).fold(V3::default(), |sum, i| {
            sum + self.world[face][i] * weights[i]
        });
        let normal = (0..3)
            .fold(V3::default(), |sum, i| {
                sum + geometry.normals[t[i]] * weights[i]
            })
            .normalized();
        Sample {
            point,
            world,
            face,
            normal,
            value,
        }
    }
    pub fn at(
        &self,
        p: P,
        values: &fields::Values,
        geometry: &fields::Geometry,
        mesh: &Mesh,
    ) -> Option<Sample> {
        self.candidates(BBox { min: p, max: p })
            .into_iter()
            .filter(|i| contains(p, &self.regions[*i].poly))
            .map(|i| self.sample(i, p, values, geometry, mesh))
            .min_by(|a, b| a.value.total_cmp(&b.value).then(a.face.cmp(&b.face)))
    }
    pub fn segments(
        &self,
        a: P,
        b: P,
        values: &fields::Values,
        geometry: &fields::Geometry,
        mesh: &Mesh,
    ) -> Vec<Segment> {
        let mut bounds = BBox::empty();
        bounds.add(a);
        bounds.add(b);
        let intervals = self
            .candidates(bounds)
            .into_iter()
            .filter_map(|i| interval(a, b, &self.regions[i].poly).map(|(lo, hi)| (lo, hi, i)))
            .collect::<Vec<_>>();
        let mut cuts = intervals
            .iter()
            .flat_map(|(lo, hi, _)| [*lo, *hi])
            .collect::<Vec<_>>();
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
        let mut out = Vec::new();
        for pair in cuts.windows(2) {
            let (lo, hi) = (pair[0], pair[1]);
            if hi - lo < 1e-10 {
                continue;
            }
            let mid = (lo + hi) * 0.5;
            let owner = intervals
                .iter()
                .filter(|(lo, hi, _)| *lo <= mid && *hi >= mid)
                .map(|(_, _, i)| (*i, self.sample(*i, a.lerp(b, mid), values, geometry, mesh)))
                .min_by(|a, b| {
                    a.1.value
                        .total_cmp(&b.1.value)
                        .then(a.1.face.cmp(&b.1.face))
                });
            if let Some((id, _)) = owner {
                out.push(Segment {
                    a: self.sample(id, a.lerp(b, lo), values, geometry, mesh),
                    b: self.sample(id, a.lerp(b, hi), values, geometry, mesh),
                });
            }
        }
        out
    }
}
pub(super) fn barycentric(p: P, [a, b, c]: [P; 3]) -> [f64; 3] {
    let det = cross(b - a, c - a);
    if det.abs() < 1e-14 {
        return [1., 0., 0.];
    }
    let u = cross(p - a, c - a) / det;
    let v = cross(b - a, p - a) / det;
    [1. - u - v, u, v]
}
fn cross(a: P, b: P) -> f64 {
    a.x * b.y - a.y * b.x
}
fn overlap(a: BBox, b: BBox) -> bool {
    a.min.x <= b.max.x + 1e-7
        && a.max.x + 1e-7 >= b.min.x
        && a.min.y <= b.max.y + 1e-7
        && a.max.y + 1e-7 >= b.min.y
}
fn contains(p: P, poly: &[P]) -> bool {
    let area: f64 = poly
        .iter()
        .zip(poly.iter().cycle().skip(1))
        .take(poly.len())
        .map(|(a, b)| cross(*a, *b))
        .sum();
    poly.iter()
        .zip(poly.iter().cycle().skip(1))
        .take(poly.len())
        .all(|(a, b)| cross(*b - *a, p - *a) * area.signum() >= -1e-8 * (*b - *a).len())
}
pub(super) fn interval(a: P, b: P, poly: &[P]) -> Option<(f64, f64)> {
    if poly.len() < 3 {
        return None;
    }
    let area: f64 = poly
        .iter()
        .zip(poly.iter().cycle().skip(1))
        .take(poly.len())
        .map(|(a, b)| cross(*a, *b))
        .sum();
    if area.abs() < 1e-14 {
        return None;
    }
    let (mut lo, mut hi) = (0f64, 1f64);
    for (&p, &q) in poly
        .iter()
        .zip(poly.iter().cycle().skip(1))
        .take(poly.len())
    {
        let d = q - p;
        let len = d.len();
        if len < 1e-12 {
            continue;
        }
        let snap = |v: f64| if v.abs() < 1e-8 { 0. } else { v };
        let start = snap(cross(d, a - p) * area.signum() / len);
        let delta = snap(cross(d, b - p) * area.signum() / len) - start;
        if delta.abs() < 1e-14 {
            if start < -1e-9 {
                return None;
            }
        } else {
            let t = -start / delta;
            if delta > 0. {
                lo = lo.max(t);
            } else {
                hi = hi.min(t);
            }
        }
        if hi - lo <= 1e-10 {
            return None;
        }
    }
    Some((lo, hi))
}
impl Surface {
    pub(super) fn isolines(
        &self,
        levels: &[f64],
        values: &fields::Values,
        geometry: &fields::Geometry,
        mesh: &Mesh,
        settings: &Settings,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Vec<Vec<[P; 2]>>, String> {
        let mut out = vec![Vec::new(); levels.len()];
        let mut budget = 0usize;
        for (id, region) in self.regions.iter().enumerate() {
            if cancel() {
                return Err("Cálculo cancelado.".into());
            }
            let samples = region
                .poly
                .iter()
                .map(|q| {
                    let mut sample = self.sample(id, *q, values, geometry, mesh);
                    // Contornos de luz interpolan las normales en zonas iluminadas y respetan
                    // las regiones analíticas de sombra arrojada, que permanecen a cero.
                    if region.intensity.is_some_and(|v| v > 0.) && values.light_weight != 0. {
                        let weights = self.camera.weights(
                            barycentric(*q, self.triangles[region.face]),
                            self.world[region.face],
                        );
                        let triangle = mesh.triangles[region.face];
                        if let Some(vertex) = &values.light_vertex {
                            let light = (0..3)
                                .map(|i| weights[i] * vertex[triangle[i]])
                                .sum::<f64>();
                            sample.value +=
                                (light - region.intensity.unwrap()) * values.light_weight;
                        }
                    }
                    sample
                })
                .collect::<Vec<_>>();
            let min = samples
                .iter()
                .map(|v| v.value)
                .fold(f64::INFINITY, f64::min);
            let max = samples
                .iter()
                .map(|v| v.value)
                .fold(f64::NEG_INFINITY, f64::max);
            if max - min < 1e-12 {
                continue;
            }
            let circular =
                values.config.measure == Measure::Aspect && values.config.secondary.is_none();
            for (index, &level) in levels.iter().enumerate() {
                for shift in if circular {
                    &[-360., 0., 360.][..]
                } else {
                    &[0.][..]
                } {
                    let level = level + shift;
                    if level < min - 1e-10 || level > max + 1e-10 {
                        continue;
                    }
                    let mut intersections = Vec::new();
                    for (a, b) in samples
                        .iter()
                        .zip(samples.iter().cycle().skip(1))
                        .take(samples.len())
                    {
                        let (va, vb) = (a.value - level, b.value - level);
                        if va.abs() < 1e-10 {
                            intersections.push(a.point);
                        }
                        if va * vb < 0. {
                            let t = va / (va - vb);
                            let q = a.world + (b.world - a.world) * t;
                            let projected = self.camera.project(q).0;
                            intersections.push(
                                self.camera.paper_center
                                    + (projected - self.camera.paper_center)
                                        .rotate(settings.paper_rotation.to_radians()),
                            );
                        }
                    }
                    intersections.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
                    intersections.dedup_by(|a, b| a.dist(*b) < 1e-7);
                    if intersections.len() >= 2 {
                        let mut far = (0., [intersections[0], intersections[1]]);
                        for &a in &intersections {
                            for &b in &intersections {
                                if a.dist(b) > far.0 {
                                    far = (a.dist(b), [a, b]);
                                }
                            }
                        }
                        if far.0 > 1e-7 {
                            out[index].push(far.1);
                            budget += 1;
                            if budget > 250_000 {
                                return Err("Las isolíneas superan 250.000 segmentos; reduce niveles o facetas.".into());
                            }
                        }
                    }
                }
            }
        }
        Ok(out)
    }
}
