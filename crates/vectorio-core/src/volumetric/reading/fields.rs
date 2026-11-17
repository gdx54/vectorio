//! Campos por vértice. Curvatura cotangente con masa baricéntrica y defecto angular.
use super::*;
use std::{collections::HashMap, sync::OnceLock};
pub(super) struct Geometry {
    pub normals: Vec<V3>,
    pub mean: Vec<f64>,
    pub gaussian: Vec<f64>,
    incident: Vec<Vec<usize>>,
    bvh: OnceLock<rays::Bvh>,
}
impl Geometry {
    fn new(mesh: &Mesh, cancel: &dyn Fn() -> bool) -> Result<Self, String> {
        let points = mesh.points();
        let n = points.len();
        let face_normals = super::super::shadow::normals(mesh, cancel)?;
        let mut normals = vec![V3::default(); n];
        let mut mass = vec![0.; n];
        let mut angles = vec![0.; n];
        let mut lap = vec![V3::default(); n];
        let mut incident = vec![vec![]; n];
        let mut edges = HashMap::<(usize, usize), usize>::with_capacity(mesh.triangles.len() * 2);
        for (i, t) in mesh.triangles.iter().enumerate() {
            if i % 128 == 0 && cancel() {
                return Err("Cálculo cancelado.".into());
            }
            let [a, b, c] = t.map(|i| points[i]);
            let area2 = (b - a).cross(c - a).length();
            if area2 < 1e-14 {
                continue;
            }
            for j in 0..3 {
                let x = t[j];
                let y = t[(j + 1) % 3];
                let z = t[(j + 2) % 3];
                let u = points[y] - points[x];
                let v = points[z] - points[x];
                let dot = u.dot(v);
                let cross = u.cross(v).length();
                let angle = cross.atan2(dot);
                angles[x] += angle;
                mass[x] += area2 / 6.;
                normals[x] = normals[x] + face_normals[i] * area2;
                incident[x].push(i);
                let cot = (dot / cross.max(1e-14)).clamp(-1e8, 1e8);
                lap[y] = lap[y] + (points[z] - points[y]) * cot;
                lap[z] = lap[z] + (points[y] - points[z]) * cot;
                *edges.entry((x.min(y), x.max(y))).or_default() += 1;
            }
        }
        for normal in &mut normals {
            *normal = normal.normalized();
        }
        let mut boundary = vec![false; n];
        for ((a, b), count) in edges {
            if count != 2 {
                boundary[a] = true;
                boundary[b] = true;
            }
        }
        let mean = (0..n)
            .map(|i| {
                if mass[i] > 1e-14 && !boundary[i] {
                    -lap[i].dot(normals[i]) / (4. * mass[i])
                } else {
                    0.
                }
            })
            .collect();
        let gaussian = (0..n)
            .map(|i| {
                if mass[i] > 1e-14 && !boundary[i] {
                    (std::f64::consts::TAU - angles[i]) / mass[i]
                } else {
                    0.
                }
            })
            .collect();
        Ok(Self {
            normals,
            mean,
            gaussian,
            incident,
            bvh: OnceLock::new(),
        })
    }
    fn rays(&self, mesh: &Mesh, cancel: &dyn Fn() -> bool) -> Result<&rays::Bvh, String> {
        if self.bvh.get().is_none() {
            // Una cancelación no deja un resultado fallido guardado para siempre.
            let bvh = rays::Bvh::new(mesh, cancel)?;
            let _ = self.bvh.set(bvh);
        }
        Ok(self.bvh.get().unwrap())
    }
}

#[cfg(test)]
mod lazy_ray_tests {
    use super::*;
    #[test]
    fn geometry_defers_ray_index_and_retries_after_cancellation() {
        let mesh = Primitive::Sphere.mesh();
        let geometry = Geometry::new(&mesh, &|| false).unwrap();
        assert!(geometry.bvh.get().is_none());
        assert!(geometry.rays(&mesh, &|| true).is_err());
        assert!(geometry.bvh.get().is_none());
        let a = geometry.rays(&mesh, &|| false).unwrap() as *const rays::Bvh;
        let b = geometry.rays(&mesh, &|| true).unwrap() as *const rays::Bvh;
        assert_eq!(a, b);
    }
}
struct FieldEntry {
    kind: Measure,
    samples: usize,
    key: [f64; 4],
    values: Arc<Vec<f64>>,
}
struct Entry {
    mesh: Arc<Mesh>,
    geometry: Arc<Geometry>,
    fields: Vec<FieldEntry>,
}
#[derive(Default)]
pub(super) struct Cache {
    entries: Vec<Entry>,
    #[cfg(test)]
    pub builds: usize,
}
impl Cache {
    pub fn geometry(
        &mut self,
        mesh: Arc<Mesh>,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Arc<Geometry>, String> {
        if let Some(e) = self.entries.iter().find(|e| Arc::ptr_eq(&e.mesh, &mesh)) {
            return Ok(e.geometry.clone());
        }
        let geometry = Arc::new(Geometry::new(&mesh, cancel)?);
        if self.entries.len() >= 6 {
            self.entries.remove(0);
        }
        self.entries.push(Entry {
            mesh,
            geometry: geometry.clone(),
            fields: vec![],
        });
        Ok(geometry)
    }
    pub fn values(
        &mut self,
        mesh: Arc<Mesh>,
        kind: Measure,
        samples: usize,
        s: &Settings,
        camera: Camera,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Arc<Vec<f64>>, String> {
        let geometry = self.geometry(mesh.clone(), cancel)?;
        let key = match kind {
            Measure::Depth => [
                camera.yaw_deg,
                camera.elev_deg,
                camera.perspective.map_or(0., |p| p[0]),
                camera.perspective.map_or(0., |p| p[1]),
            ],
            Measure::Light => [s.shadow.azimuth, s.shadow.elevation, 0., 0.],
            _ => [0.; 4],
        };
        let relevant_samples = if kind == Measure::AmbientOcclusion {
            samples
        } else {
            0
        };
        let index = self
            .entries
            .iter()
            .position(|e| Arc::ptr_eq(&e.mesh, &mesh))
            .unwrap();
        if let Some(v) = self.entries[index]
            .fields
            .iter()
            .find(|v| v.kind == kind && v.samples == relevant_samples && v.key == key)
        {
            return Ok(v.values.clone());
        }
        let points = mesh.points();
        let values = match kind {
            Measure::Height => points.iter().map(|p| p.z).collect(),
            Measure::Depth => points.iter().map(|p| camera.project(*p).1).collect(),
            Measure::Geodesic => super::super::art::field::Graph::new(&mesh, cancel)?
                .distance(V3::new(0., 0., 1.), cancel)?,
            Measure::Light => geometry
                .normals
                .iter()
                .map(|n| n.dot(s.shadow.light()).max(0.))
                .collect(),
            Measure::MeanCurvature => geometry.mean.clone(),
            Measure::GaussianCurvature => geometry.gaussian.clone(),
            Measure::Slope => geometry
                .normals
                .iter()
                .map(|n| n.z.abs().clamp(0., 1.).acos().to_degrees())
                .collect(),
            Measure::Aspect => geometry
                .normals
                .iter()
                .map(|n| n.x.atan2(n.y).to_degrees().rem_euclid(360.))
                .collect(),
            Measure::AmbientOcclusion | Measure::Thickness => {
                let bvh = geometry.rays(&mesh, cancel)?;
                let extent = (mesh.bounds()[1] - mesh.bounds()[0]).length().max(1e-6);
                let epsilon = extent * 1e-6;
                let mut values = Vec::with_capacity(points.len());
                for (i, &point) in points.iter().enumerate() {
                    if cancel() {
                        return Err("Cálculo cancelado.".into());
                    }
                    let n = geometry.normals[i];
                    if kind == Measure::Thickness {
                        values.push(
                            bvh.nearest(
                                point - n * epsilon,
                                n * (-1.),
                                &geometry.incident[i],
                                extent * 2.,
                            )
                            .map_or(0., |v| v + epsilon),
                        );
                        continue;
                    }
                    let tangent = if n.z.abs() < 0.9 {
                        n.cross(V3::new(0., 0., 1.)).normalized()
                    } else {
                        n.cross(V3::new(1., 0., 0.)).normalized()
                    };
                    let bitangent = n.cross(tangent);
                    let mut hits = 0;
                    for j in 0..samples {
                        if j % 16 == 0 && cancel() {
                            return Err("Cálculo cancelado.".into());
                        }
                        let u = (j as f64 + 0.5) / samples as f64;
                        let phi = j as f64 * 2.399963229728653;
                        let d = tangent * (u.sqrt() * phi.cos())
                            + bitangent * (u.sqrt() * phi.sin())
                            + n * (1. - u).sqrt();
                        hits += usize::from(
                            bvh.nearest(point + n * epsilon, d, &geometry.incident[i], extent * 2.)
                                .is_some(),
                        );
                    }
                    values.push(hits as f64 / samples as f64);
                }
                values
            }
            Measure::Objects => vec![0.; points.len()],
        };
        if values.iter().any(|v| !v.is_finite()) {
            return Err("La medida produjo valores no finitos; revisa la malla.".into());
        }
        let values = Arc::new(values);
        let fields = &mut self.entries[index].fields;
        if fields.len() >= 24 {
            fields.remove(0);
        }
        fields.push(FieldEntry {
            kind,
            samples: relevant_samples,
            key,
            values: values.clone(),
        });
        #[cfg(test)]
        {
            self.builds += 1;
        }
        Ok(values)
    }
}
#[derive(Clone)]
pub(super) struct Values {
    pub values: Arc<Vec<f64>>,
    pub range: [f64; 2],
    pub sorted: Vec<f64>,
    pub config: ReadingMap,
    pub light_vertex: Option<Arc<Vec<f64>>>,
    pub light_weight: f64,
}
impl Values {
    pub fn new(
        cache: &mut Cache,
        d: &Drawing,
        s: &Settings,
        config: &ReadingMap,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Self, String> {
        let first = cache.values(
            d.mesh.clone(),
            config.measure,
            config.samples,
            s,
            d.camera,
            cancel,
        )?;
        let values = if let Some(second) = config.secondary {
            let second =
                cache.values(d.mesh.clone(), second, config.samples, s, d.camera, cancel)?;
            Arc::new(
                first
                    .iter()
                    .zip(second.iter())
                    .map(|(a, b)| a * config.weights[0] + b * config.weights[1])
                    .collect(),
            )
        } else {
            first
        };
        let light_weight = if config.secondary.is_none() {
            if config.measure == Measure::Light {
                1.
            } else {
                0.
            }
        } else {
            (if config.measure == Measure::Light {
                config.weights[0]
            } else {
                0.
            }) + (if config.secondary == Some(Measure::Light) {
                config.weights[1]
            } else {
                0.
            })
        };
        let light_vertex = if light_weight != 0. {
            Some(cache.values(
                d.mesh.clone(),
                Measure::Light,
                config.samples,
                s,
                d.camera,
                cancel,
            )?)
        } else {
            None
        };
        let mut range = config.range.unwrap_or_else(|| {
            if config.secondary.is_none() {
                match config.measure {
                    Measure::Light | Measure::AmbientOcclusion => return [0., 1.],
                    Measure::Slope => return [0., 90.],
                    Measure::Aspect => return [0., 360.],
                    _ => {}
                }
            }
            let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            [lo, hi]
        });
        if config.ramp == Ramp::Diverging {
            let extent = range[0].abs().max(range[1].abs()).max(1e-9);
            range = [-extent, extent];
        }
        let mut sorted = Vec::new();
        if config.equalize {
            sorted = values.as_ref().clone();
            sorted.sort_by(f64::total_cmp);
        }
        Ok(Self {
            values,
            range,
            sorted,
            config: config.clone(),
            light_vertex,
            light_weight,
        })
    }
    pub fn normalize(&self, value: f64) -> f64 {
        let value = if self.config.measure == Measure::Aspect && self.config.secondary.is_none() {
            let center = (self.range[0] + self.range[1]) * 0.5;
            center + (value - center + 180.).rem_euclid(360.) - 180.
        } else {
            value
        };
        let mut t = if let Some(threshold) = self.config.threshold {
            if value >= threshold {
                1.
            } else {
                0.
            }
        } else if self.config.equalize && !self.sorted.is_empty() {
            if self.config.ramp == Ramp::Diverging {
                let zero = self.sorted.partition_point(|v| *v < 0.);
                let positive = self.sorted.partition_point(|v| *v <= 0.);
                if value < 0. {
                    let count = self.sorted[..zero].partition_point(|v| *v <= value);
                    0.5 * count as f64 / zero.max(1) as f64
                } else if value > 0. {
                    let count = self.sorted[positive..].partition_point(|v| *v < value);
                    0.5 + 0.5 * count as f64 / (self.sorted.len() - positive).max(1) as f64
                } else {
                    0.5
                }
            } else {
                let lo = self.sorted.partition_point(|v| *v < value);
                let hi = self.sorted.partition_point(|v| *v <= value);
                (lo + hi) as f64 / (2. * self.sorted.len() as f64)
            }
        } else if self.range[1] - self.range[0] > 1e-12 {
            ((value - self.range[0]) / (self.range[1] - self.range[0])).clamp(0., 1.)
        } else {
            0.5
        };
        if self.config.invert {
            t = 1. - t;
        }
        t
    }
    pub fn raw_at(&self, t: f64) -> f64 {
        let t = if self.config.invert { 1. - t } else { t };
        if self.config.equalize && !self.sorted.is_empty() {
            if self.config.ramp == Ramp::Diverging {
                if t == 0.5 {
                    return 0.;
                }
                let zero = self.sorted.partition_point(|v| *v < 0.);
                let positive = self.sorted.partition_point(|v| *v <= 0.);
                if t < 0.5 {
                    return if zero == 0 {
                        0.
                    } else {
                        self.sorted[(t * 2. * zero as f64).floor().min((zero - 1) as f64) as usize]
                    };
                }
                let count = self.sorted.len() - positive;
                return if count == 0 {
                    0.
                } else {
                    self.sorted[positive
                        + (((t - 0.5) * 2. * count as f64).floor() as usize).min(count - 1)]
                };
            }
            self.sorted[(t * self.sorted.len() as f64)
                .floor()
                .min((self.sorted.len() - 1) as f64) as usize]
        } else {
            self.range[0] + (self.range[1] - self.range[0]) * t
        }
    }
}
