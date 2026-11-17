//! Recorte analítico de regiones convexas contra triángulos más cercanos.
//! La misma operación sirve para visibilidad de cámara y oclusión de la luz.
use crate::scene::V3;
use crate::{p, BBox, P};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Vertex {
    pub world: V3,
    pub point: P,
    pub depth: f64,
}
impl Vertex {
    fn lerp(self, b: Self, t: f64) -> Self {
        Self {
            world: self.world + (b.world - self.world) * t,
            point: self.point.lerp(b.point, t),
            depth: self.depth + (b.depth - self.depth) * t,
        }
    }
}
pub(crate) type Polygon = Vec<Vertex>;
#[derive(Clone, Copy)]
pub(crate) struct Plane {
    n: P,
    c: f64,
}
impl Plane {
    fn value(self, q: P) -> f64 {
        self.n.dot(q) + self.c
    }
}
#[derive(Clone)]
pub(crate) struct Triangle {
    pub polygon: Polygon,
    pub bounds: BBox,
    gradient: P,
    constant: f64,
    pub min: f64,
    pub max: f64,
    planes: Vec<Plane>,
}
/// Las caras pequeñas detrás de muchas otras se parten en astillas sin tamaño visible.
/// En vez de fallar, se descartan y, si aún sobran, se conservan las más grandes.
const FRAGMENTS: usize = 512;
const CAST_FRAGMENTS: usize = 16_384;
fn area2(poly: &[Vertex]) -> f64 {
    poly.iter()
        .zip(poly.iter().cycle().skip(1))
        .take(poly.len())
        .map(|(a, b)| cross(a.point, b.point))
        .sum::<f64>()
        .abs()
}
fn world_area(poly: &[V3]) -> f64 {
    (1..poly.len().saturating_sub(1))
        .map(|i| (poly[i] - poly[0]).cross(poly[i + 1] - poly[0]).length())
        .sum()
}
fn prune(polygons: &mut Vec<Polygon>, whole: f64) {
    polygons.retain(|q| area2(q) > whole * 1e-9);
    if polygons.len() > FRAGMENTS {
        polygons.sort_by(|a, b| area2(b).total_cmp(&area2(a)));
        polygons.truncate(FRAGMENTS);
    }
}
fn cross(a: P, b: P) -> f64 {
    a.x * b.y - a.y * b.x
}
pub(crate) fn bounds(poly: &[Vertex]) -> BBox {
    let mut b = BBox::empty();
    for v in poly {
        b.add(v.point);
    }
    b
}
fn overlap(a: BBox, b: BBox) -> bool {
    a.min.x <= b.max.x + 1e-9
        && a.max.x >= b.min.x - 1e-9
        && a.min.y <= b.max.y + 1e-9
        && a.max.y >= b.min.y - 1e-9
}
pub(crate) fn boundary(poly: &[Vertex]) -> Vec<Plane> {
    let area: f64 = poly
        .iter()
        .zip(poly.iter().cycle().skip(1))
        .take(poly.len())
        .map(|(a, b)| cross(a.point, b.point))
        .sum();
    let sign = area.signum();
    poly.iter()
        .zip(poly.iter().cycle().skip(1))
        .take(poly.len())
        .filter_map(|(a, b)| {
            let d = b.point - a.point;
            let len = d.len();
            if len < 1e-12 {
                return None;
            }
            let n = p(-d.y, d.x) * (sign / len);
            Some(Plane {
                n,
                c: -n.dot(a.point),
            })
        })
        .collect()
}
impl Triangle {
    fn new(poly: Polygon) -> Option<Self> {
        let a = poly[0];
        let u = poly[1].point - a.point;
        let v = poly[2].point - a.point;
        let det = cross(u, v);
        if det.abs() < 1e-12 {
            return None;
        }
        let du = poly[1].depth - a.depth;
        let dv = poly[2].depth - a.depth;
        let gradient = p((du * v.y - dv * u.y) / det, (u.x * dv - v.x * du) / det);
        let constant = a.depth - gradient.dot(a.point);
        Some(Self {
            bounds: bounds(&poly),
            min: poly.iter().map(|v| v.depth).fold(f64::INFINITY, f64::min),
            max: poly
                .iter()
                .map(|v| v.depth)
                .fold(f64::NEG_INFINITY, f64::max),
            planes: boundary(&poly),
            polygon: poly,
            gradient,
            constant,
        })
    }
    fn occlusion(&self, receiver: &Triangle) -> Vec<Plane> {
        let mut planes = self.planes.clone();
        planes.push(Plane {
            n: receiver.gradient - self.gradient,
            c: receiver.constant - self.constant - 1e-7,
        });
        planes
    }
}
/// Partición sin pérdidas de área; ambos lados comparten exactamente el corte.
fn split(poly: &[Vertex], plane: Plane) -> (Polygon, Polygon) {
    let (mut inside, mut outside) = (Vec::new(), Vec::new());
    if poly.is_empty() {
        return (inside, outside);
    }
    for (&a, &b) in poly
        .iter()
        .zip(poly.iter().cycle().skip(1))
        .take(poly.len())
    {
        let va = plane.value(a.point);
        let vb = plane.value(b.point);
        if va >= 0. {
            inside.push(a);
        } else {
            outside.push(a);
        }
        if (va >= 0.) != (vb >= 0.) {
            let q = a.lerp(b, (va / (va - vb)).clamp(0., 1.));
            inside.push(q);
            outside.push(q);
        }
    }
    (clean(inside), clean(outside))
}
fn clean(mut poly: Polygon) -> Polygon {
    poly.dedup_by(|a, b| a.point.dist(b.point) < 1e-10);
    if poly.len() > 1 && poly[0].point.dist(poly.last().unwrap().point) < 1e-10 {
        poly.pop();
    }
    if poly.len() < 3 {
        poly.clear();
    }
    poly
}
pub(crate) fn intersection(poly: &[Vertex], planes: &[Plane]) -> Polygon {
    let mut poly = poly.to_vec();
    for &plane in planes {
        poly = split(&poly, plane).0;
        if poly.is_empty() {
            break;
        }
    }
    poly
}
pub(crate) fn subtract(poly: &[Vertex], planes: &[Plane]) -> Vec<Polygon> {
    if intersection(poly, planes).is_empty() {
        return vec![poly.to_vec()];
    }
    let mut remainder = poly.to_vec();
    let mut pieces = Vec::new();
    for &plane in planes {
        let (inside, outside) = split(&remainder, plane);
        if !outside.is_empty() {
            pieces.push(outside);
        }
        remainder = inside;
        if remainder.is_empty() {
            break;
        }
    }
    pieces
}

struct Node {
    bounds: BBox,
    start: usize,
    end: usize,
    children: Option<(usize, usize)>,
}
pub(crate) struct Projection {
    pub faces: Vec<Option<Triangle>>,
    ids: Vec<usize>,
    nodes: Vec<Node>,
}
impl Projection {
    pub fn new(mesh: &super::Mesh, project: &dyn Fn(V3) -> (P, f64)) -> Self {
        Self::build_mesh(mesh, project, &|p| p.to_vec())
    }
    pub fn camera(mesh: &super::Mesh, camera: &crate::volumetric::Camera, rotation: f64) -> Self {
        Self::build_mesh(
            mesh,
            &|q| {
                let (p, z) = camera.project(q);
                (
                    camera.paper_center + (p - camera.paper_center).rotate(rotation.to_radians()),
                    z,
                )
            },
            &|poly| camera.clip_polygon(poly),
        )
    }
    fn build_mesh(
        mesh: &super::Mesh,
        project: &dyn Fn(V3) -> (P, f64),
        clip: &dyn Fn(&[V3]) -> Vec<V3>,
    ) -> Self {
        let points = mesh.points();
        let faces: Vec<_> = mesh
            .triangles
            .iter()
            .map(|t| {
                let world = clip(&t.map(|i| points[i]));
                if world.len() < 3 {
                    return None;
                }
                Triangle::new(
                    world
                        .into_iter()
                        .map(|world| {
                            let (point, depth) = project(world);
                            Vertex {
                                world,
                                point,
                                depth,
                            }
                        })
                        .collect(),
                )
            })
            .collect();
        let ids = faces
            .iter()
            .enumerate()
            .filter(|(_, t)| t.is_some())
            .map(|(i, _)| i)
            .collect();
        let mut s = Self {
            faces,
            ids,
            nodes: vec![],
        };
        if !s.ids.is_empty() {
            s.build(0, s.ids.len());
        }
        s
    }
    fn build(&mut self, start: usize, end: usize) -> usize {
        let mut bounds = BBox::empty();
        for &i in &self.ids[start..end] {
            let b = self.faces[i].as_ref().unwrap().bounds;
            bounds.add(b.min);
            bounds.add(b.max);
        }
        let node = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            start,
            end,
            children: None,
        });
        if end - start > 12 {
            let x = bounds.width() >= bounds.height();
            self.ids[start..end].sort_by(|a, b| {
                let a = self.faces[*a].as_ref().unwrap().bounds.center();
                let b = self.faces[*b].as_ref().unwrap().bounds.center();
                (if x { a.x } else { a.y }).total_cmp(&if x { b.x } else { b.y })
            });
            let mid = (start + end) / 2;
            let left = self.build(start, mid);
            let right = self.build(mid, end);
            self.nodes[node].children = Some((left, right));
        }
        node
    }
    fn candidates(&self, bbox: BBox) -> Vec<usize> {
        let mut out = Vec::new();
        if self.nodes.is_empty() {
            return out;
        }
        let mut stack = vec![0];
        while let Some(i) = stack.pop() {
            let node = &self.nodes[i];
            if !overlap(node.bounds, bbox) {
                continue;
            }
            if let Some((a, b)) = node.children {
                stack.extend([a, b]);
            } else {
                out.extend(
                    self.ids[node.start..node.end]
                        .iter()
                        .copied()
                        .filter(|i| overlap(self.faces[*i].as_ref().unwrap().bounds, bbox)),
                );
            }
        }
        out.sort_unstable();
        out
    }
    pub fn visible(&self, face: usize, cancel: &dyn Fn() -> bool) -> Result<Vec<Polygon>, String> {
        let Some(receiver) = &self.faces[face] else {
            return Ok(vec![]);
        };
        let mut polygons = vec![receiver.polygon.clone()];
        for other in self.candidates(receiver.bounds) {
            if cancel() {
                return Err(super::CANCEL.into());
            }
            if other == face {
                continue;
            }
            let occluder = self.faces[other].as_ref().unwrap();
            if occluder.min >= receiver.max - 1e-7 {
                continue;
            }
            let planes = occluder.occlusion(receiver);
            let mut next = Vec::new();
            for poly in &polygons {
                next.extend(subtract(poly, &planes));
            }
            polygons = next;
            if polygons.len() > FRAGMENTS {
                prune(&mut polygons, area2(&receiver.polygon));
            }
            if polygons.is_empty() {
                break;
            }
        }
        Ok(polygons)
    }
    pub fn shadowed(&self, face: usize, cancel: &dyn Fn() -> bool) -> Result<Vec<Vec<V3>>, String> {
        let Some(receiver) = &self.faces[face] else {
            return Ok(vec![]);
        };
        let mut polygons: Vec<Vec<V3>> = Vec::new();
        for other in self.candidates(receiver.bounds) {
            if cancel() {
                return Err(super::CANCEL.into());
            }
            if other == face {
                continue;
            }
            let occluder = self.faces[other].as_ref().unwrap();
            if occluder.min >= receiver.max - 1e-7 {
                continue;
            }
            let poly = intersection(&receiver.polygon, &occluder.occlusion(receiver));
            if !poly.is_empty() {
                polygons.push(poly.iter().map(|v| v.world).collect());
            }
            if polygons.len() > CAST_FRAGMENTS {
                // Una cara grande (la peana) recibe la sombra de miles de caras pequeñas: se
                // conservan todas salvo las astillas; los límites globales protegen la memoria.
                let whole = world_area(&receiver.polygon.iter().map(|v| v.world).collect::<Vec<_>>());
                polygons.retain(|q| world_area(q) > 1e-9 * whole);
                if polygons.len() > CAST_FRAGMENTS {
                    polygons.sort_by(|a, b| world_area(b).total_cmp(&world_area(a)));
                    polygons.truncate(CAST_FRAGMENTS);
                }
            }
        }
        Ok(polygons)
    }
}
