//! Campo tonal por caras, sombras de luz y recorte vectorial de cualquier pasada.
//! No muestrea píxeles: conserva las fronteras analíticas en milímetros.
use super::*;
use crate::volumetric::{DashPattern, HatchMode, TonalMask, ToneHatchParams};

#[derive(Clone)]
struct Region {
    value: f64,
    polygon: Polyline,
    /// Valor en cada vértice cuando el tono varía dentro de la región; vacío si es constante.
    values: Vec<f64>,
}
/// Corta las regiones con gradiente en los niveles pedidos. Cada pieza devuelta es
/// constante respecto a esos niveles, así que su valor medio la clasifica bien.
fn bands(regions: &[Region], levels: &[f64]) -> Vec<Region> {
    let mut levels = levels.to_vec();
    levels.sort_by(f64::total_cmp);
    levels.dedup();
    let mut out = Vec::with_capacity(regions.len());
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    for r in regions {
        if r.values.is_empty() {
            out.push(r.clone());
            continue;
        }
        let lo = r.values.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = r.values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let cuts: Vec<f64> = levels
            .iter()
            .copied()
            .filter(|l| *l > lo && *l < hi)
            .collect();
        if cuts.is_empty() {
            out.push(Region {
                value: r.value,
                polygon: r.polygon.clone(),
                values: Vec::new(),
            });
            continue;
        }
        let bounds: Vec<f64> = std::iter::once(f64::NEG_INFINITY)
            .chain(cuts)
            .chain(std::iter::once(f64::INFINITY))
            .collect();
        for w in bounds.windows(2) {
            let (polygon, values) = slab(&r.polygon, &r.values, w[0], w[1]);
            if !polygon.is_empty() {
                out.push(Region {
                    value: mean(&values),
                    polygon,
                    values: Vec::new(),
                });
            }
        }
    }
    out
}
struct Entry {
    mesh: Arc<Mesh>,
    camera: Camera,
    rotation: f64,
    light: (f64, f64),
    regions: Arc<Vec<Region>>,
}
#[derive(Default)]
pub(crate) struct ToneCache {
    light: Cache,
    entries: Vec<Entry>,
    #[cfg(test)]
    pub builds: usize,
}
impl ToneCache {
    fn regions(
        &mut self,
        drawing: &Drawing,
        settings: &Settings,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Arc<Vec<Region>>, String> {
        let key = (settings.shadow.azimuth, settings.shadow.elevation);
        if let Some(e) = self.entries.iter().find(|e| {
            Arc::ptr_eq(&e.mesh, &drawing.mesh)
                && e.camera == drawing.camera
                && e.rotation == settings.paper_rotation
                && e.light == key
        }) {
            return Ok(e.regions.clone());
        }
        let config = ShadowSettings {
            threshold: 0.,
            middle_tone: false,
            ..settings.shadow.clone()
        };
        let faces = self.light.field(drawing.mesh.clone(), &config, cancel)?;
        if self.light.fields.len() > 16 {
            self.light.fields.drain(..self.light.fields.len() - 16);
        }
        let camera = drawing.camera;
        let project = |q| {
            let (point, depth) = camera.project(q);
            (
                camera.paper_center
                    + (point - camera.paper_center).rotate(settings.paper_rotation.to_radians()),
                depth,
            )
        };
        let projection =
            project::Projection::camera(&drawing.mesh, &camera, settings.paper_rotation);
        let points = drawing.mesh.points();
        let mut regions = Vec::new();
        for (i, face) in faces.iter().enumerate() {
            if cancel() {
                return Err(CANCEL.into());
            }
            let mut visible = projection.visible(i, cancel)?;
            let tri = drawing.mesh.triangles[i].map(|v| points[v]);
            let corners = face.corners.map(|c| c.clamp(0., 1.));
            for patch in &face.cast {
                let region = project_polygon(patch, &project);
                let planes = project::boundary(&region);
                let mut lit = Vec::new();
                for polygon in &visible {
                    let dark = project::intersection(polygon, &planes);
                    if !dark.is_empty() {
                        regions.push(Region {
                            value: 0.,
                            polygon: dark.iter().map(|v| v.point).collect(),
                            values: Vec::new(),
                        });
                    }
                    lit.extend(project::subtract(polygon, &planes));
                }
                visible = lit;
                if visible.len() + regions.len() > PIECE_LIMIT {
                    return Err("Demasiadas regiones tonales; reduce el detalle del modelo.".into());
                }
            }
            regions.extend(visible.into_iter().map(|poly| {
                let values: Vec<f64> = poly.iter().map(|v| shade(tri, corners, v.world)).collect();
                Region {
                    value: values.iter().sum::<f64>() / values.len() as f64,
                    polygon: poly.iter().map(|v| v.point).collect(),
                    values,
                }
            }));
            if regions.len() > PIECE_LIMIT {
                return Err("Demasiadas regiones tonales; reduce el detalle del modelo.".into());
            }
        }
        let regions = Arc::new(regions);
        if self.entries.len() >= 8 {
            self.entries.remove(0);
        }
        self.entries.push(Entry {
            mesh: drawing.mesh.clone(),
            camera,
            rotation: settings.paper_rotation,
            light: key,
            regions: regions.clone(),
        });
        #[cfg(test)]
        {
            self.builds += 1;
        }
        Ok(regions)
    }
    pub fn apply(
        &mut self,
        drawing: &Drawing,
        lines: &[Polyline],
        mask: TonalMask,
        settings: &Settings,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Vec<Polyline>, String> {
        if mask == TonalMask::All {
            return Ok(lines.to_vec());
        }
        let regions = self.regions(drawing, settings, cancel)?;
        let levels = match mask {
            TonalMask::Band { min, max } => vec![min, max],
            _ => vec![settings.shadow.threshold],
        };
        let regions = bands(&regions, &levels);
        Mask::new(&regions, |value| {
            accepts(mask, value, settings.shadow.threshold)
        })
        .clip(lines, cancel)
    }
    pub fn hatch(
        &mut self,
        drawing: &Drawing,
        params: &ToneHatchParams,
        settings: &Settings,
        paper: &crate::Paper,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Vec<Polyline>, String> {
        let regions = self.regions(drawing, settings, cancel)?;
        let levels: Vec<f64> = (0..params.levels)
            .map(|level| (params.levels - level) as f64 / (params.levels + 1) as f64)
            .collect();
        let regions = bands(&regions, &levels);
        let area = BBox {
            min: p(paper.margin_mm, paper.margin_mm),
            max: p(
                paper.width_mm - paper.margin_mm,
                paper.height_mm - paper.margin_mm,
            ),
        };
        let mut output = Vec::new();
        let form = if params.mode == HatchMode::Form {
            Some(form_lines(drawing, params, settings, area, cancel)?)
        } else {
            None
        };
        for level in 0..params.levels {
            if cancel() {
                return Err(CANCEL.into());
            }
            let threshold = (params.levels - level) as f64 / (params.levels + 1) as f64;
            if let Some(form) = &form {
                output.extend(Mask::new(&regions, |v| v <= threshold).clip(&form[level], cancel)?);
            } else {
                let polygons: Vec<_> = regions
                    .iter()
                    .filter(|r| r.value <= threshold)
                    .map(|r| r.polygon.clone())
                    .collect();
                let [lines, _] = mask::hatch(
                    &polygons,
                    &[],
                    &[],
                    &[],
                    area,
                    params.angles[level],
                    params.spacing_mm,
                    cancel,
                )?;
                output.extend(lines);
            }
            if output.len() > PIECE_LIMIT {
                return Err("La trama supera 250.000 trazos; reduce la densidad.".into());
            }
        }
        Ok(output)
    }
}
fn accepts(mask: TonalMask, value: f64, threshold: f64) -> bool {
    match mask {
        TonalMask::All => true,
        TonalMask::Light => value > threshold,
        TonalMask::Shadow => value <= threshold,
        TonalMask::Band { min, max } => value >= min && (value < max || max == 1. && value <= max),
    }
}
/// Secciones en el espacio del objeto; las familias se intercalan sin repasos.
fn form_lines(
    drawing: &Drawing,
    params: &ToneHatchParams,
    settings: &Settings,
    area: BBox,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Polyline>>, String> {
    let camera = drawing.camera;
    let mut render_camera = camera;
    render_camera.scale /= 1000.;
    render_camera.world_center = render_camera.world_center * 1000.;
    let rotate = |q: V3| {
        let q = q - camera.world_center;
        let axis = camera.axes().2;
        let (sin, cos) = settings.paper_rotation.to_radians().sin_cos();
        (camera.world_center + q * cos + axis.cross(q) * sin + axis * (axis.dot(q) * (1. - cos)))
            * 1000.
    };
    let mut scene = crate::scene::Scene::new(render_camera, area);
    scene.set_intersections(false);
    let parts: Vec<_> = (0..params.levels)
        .map(|i| scene.part(&format!("Nivel {i}")))
        .collect();
    let points = drawing.mesh.points();
    for t in &drawing.mesh.triangles {
        if cancel() {
            return Err(CANCEL.into());
        }
        scene.occluder(&t.map(|i| rotate(points[i])));
    }
    for curve in drawing.curves.iter() {
        let mut family = params.levels - 1;
        for level in 0..params.levels {
            let stride = 1 << (params.levels - level - 1);
            if curve.level % stride == 0 {
                family = level;
                break;
            }
        }
        scene.set_part(parts[family]);
        scene.line(&curve.points.iter().map(|q| rotate(*q)).collect::<Vec<_>>());
    }
    let rendered = scene.render_cancellable(cancel).ok_or(CANCEL)?;
    Ok(rendered
        .parts
        .into_iter()
        .skip(1)
        .map(|part| part.lines)
        .collect())
}

fn overlap(a: BBox, b: BBox) -> bool {
    a.min.x <= b.max.x + 1e-7
        && a.max.x + 1e-7 >= b.min.x
        && a.min.y <= b.max.y + 1e-7
        && a.max.y + 1e-7 >= b.min.y
}
struct Polygon {
    points: Polyline,
    bounds: BBox,
    value: f64,
    accepted: bool,
}
struct Node {
    bounds: BBox,
    start: usize,
    end: usize,
    children: Option<(usize, usize)>,
}
struct Mask {
    polygons: Vec<Polygon>,
    ids: Vec<usize>,
    nodes: Vec<Node>,
}
impl Mask {
    fn new(regions: &[Region], accepts: impl Fn(f64) -> bool) -> Self {
        let polygons: Vec<_> = regions
            .iter()
            .map(|r| {
                let mut bounds = BBox::empty();
                for p in &r.polygon {
                    bounds.add(*p);
                }
                Polygon {
                    points: r.polygon.clone(),
                    bounds,
                    value: r.value,
                    accepted: accepts(r.value),
                }
            })
            .collect();
        let ids = (0..polygons.len()).collect();
        let mut mask = Self {
            polygons,
            ids,
            nodes: Vec::new(),
        };
        if !mask.ids.is_empty() {
            mask.build(0, mask.ids.len());
        }
        mask
    }
    fn build(&mut self, start: usize, end: usize) -> usize {
        let mut bounds = BBox::empty();
        for &i in &self.ids[start..end] {
            bounds.add(self.polygons[i].bounds.min);
            bounds.add(self.polygons[i].bounds.max);
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
                let a = self.polygons[*a].bounds.center();
                let b = self.polygons[*b].bounds.center();
                (if x { a.x } else { a.y }).total_cmp(&if x { b.x } else { b.y })
            });
            let mid = (start + end) / 2;
            let a = self.build(start, mid);
            let b = self.build(mid, end);
            self.nodes[id].children = Some((a, b));
        }
        id
    }
    fn intervals(&self, a: P, b: P) -> Vec<(f64, f64)> {
        if self.nodes.is_empty() {
            return Vec::new();
        }
        let mut bounds = BBox::empty();
        bounds.add(a);
        bounds.add(b);
        let mut stack = vec![0];
        let mut intervals = Vec::new();
        while let Some(i) = stack.pop() {
            let node = &self.nodes[i];
            if !overlap(node.bounds, bounds) {
                continue;
            }
            if let Some((a, b)) = node.children {
                stack.extend([a, b]);
            } else {
                for &i in &self.ids[node.start..node.end] {
                    let poly = &self.polygons[i];
                    if overlap(poly.bounds, bounds) {
                        if let Some((lo, hi)) = polygon_interval(a, b, &poly.points) {
                            intervals.push((lo, hi, poly.value, poly.accepted));
                        }
                    }
                }
            }
        }
        // En una arista compartida manda la cara más oscura. El criterio es el
        // mismo para todas las máscaras: luz y sombra forman una partición exacta.
        let mut cuts: Vec<_> = intervals
            .iter()
            .flat_map(|(lo, hi, _, _)| [*lo, *hi])
            .collect();
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
        let mut merged: Vec<(f64, f64)> = Vec::new();
        for pair in cuts.windows(2) {
            let (lo, hi) = (pair[0], pair[1]);
            let mid = (lo + hi) * 0.5;
            let accepted = intervals
                .iter()
                .filter(|(lo, hi, _, _)| *lo <= mid && *hi >= mid)
                .min_by(|a, b| a.2.total_cmp(&b.2))
                .is_some_and(|i| i.3);
            if accepted {
                if let Some(last) = merged.last_mut().filter(|last| lo <= last.1 + 1e-9) {
                    last.1 = hi;
                } else {
                    merged.push((lo, hi));
                }
            }
        }
        merged
    }
    fn clip(&self, lines: &[Polyline], cancel: &dyn Fn() -> bool) -> Result<Vec<Polyline>, String> {
        let mut out = Vec::new();
        for line in lines {
            let mut current: Polyline = Vec::new();
            for pair in line.windows(2) {
                if cancel() {
                    return Err(CANCEL.into());
                }
                let (a, b) = (pair[0], pair[1]);
                let intervals = self.intervals(a, b);
                for &(lo, hi) in &intervals {
                    let (a, b) = (a.lerp(b, lo), a.lerp(b, hi));
                    if current.last().is_some_and(|q| q.dist(a) > 1e-8) {
                        flush(&mut current, &mut out);
                    }
                    if current.is_empty() {
                        current.push(a);
                    }
                    current.push(b);
                }
                if intervals.last().is_none_or(|(_, hi)| *hi < 1. - 1e-9) {
                    flush(&mut current, &mut out);
                }
                if out.len() > PIECE_LIMIT {
                    return Err("Demasiados fragmentos tonales; reduce la densidad.".into());
                }
            }
            flush(&mut current, &mut out);
        }
        Ok(out)
    }
}
fn flush(current: &mut Polyline, out: &mut Vec<Polyline>) {
    if current.len() >= 2 && crate::geom::length(current) >= 0.025 {
        out.push(std::mem::take(current));
    } else {
        current.clear();
    }
}
/// Intersección de un segmento con un polígono convexo, independiente del winding.
fn polygon_interval(a: P, b: P, poly: &[P]) -> Option<(f64, f64)> {
    if poly.len() < 3 {
        return None;
    }
    let cross = |a: P, b: P| a.x * b.y - a.y * b.x;
    let area: f64 = poly
        .iter()
        .zip(poly.iter().cycle().skip(1))
        .take(poly.len())
        .map(|(a, b)| cross(*a, *b))
        .sum();
    if area.abs() < 1e-14 {
        return None;
    }
    let sign = area.signum();
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
        let start = snap(cross(d, a - p) * sign / len);
        let end = snap(cross(d, b - p) * sign / len);
        let delta = end - start;
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
/// Fase de guion acumulada a lo largo de cada polilínea, en mm de papel.
pub(crate) fn dashed(
    lines: &[Polyline],
    pattern: DashPattern,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Polyline>, String> {
    let period = pattern.dash_mm + pattern.gap_mm;
    let mut out = Vec::new();
    for line in lines {
        let mut distance = 0.;
        let mut current = Vec::new();
        for segment in line.windows(2) {
            if cancel() {
                return Err(CANCEL.into());
            }
            let len = segment[0].dist(segment[1]);
            if len < 1e-12 {
                continue;
            }
            let mut at = 0.;
            while at < len - 1e-10 {
                let phase = distance % period;
                let on = phase < pattern.dash_mm - 1e-9;
                let step = (if on {
                    pattern.dash_mm - phase
                } else {
                    period - phase
                })
                .max(1e-9)
                .min(len - at);
                if on {
                    if current.is_empty() {
                        current.push(segment[0].lerp(segment[1], at / len));
                    }
                    current.push(segment[0].lerp(segment[1], (at + step) / len));
                } else {
                    flush(&mut current, &mut out);
                }
                at += step;
                distance += step;
                if out.len() > PIECE_LIMIT {
                    return Err("Demasiados guiones; aumenta su longitud.".into());
                }
            }
        }
        flush(&mut current, &mut out);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rectangle(lo: f64, hi: f64, value: f64) -> Region {
        Region {
            value,
            polygon: vec![p(lo, 0.), p(hi, 0.), p(hi, 10.), p(lo, 10.)],
            values: Vec::new(),
        }
    }
    #[test]
    fn a_gradient_region_is_cut_exactly_at_each_level() {
        let ramp = Region {
            value: 0.5,
            polygon: vec![p(0., 0.), p(10., 0.), p(10., 10.), p(0., 10.)],
            values: vec![0., 1., 1., 0.],
        };
        let pieces = bands(&[ramp], &[0.25, 0.75]);
        assert_eq!(pieces.len(), 3);
        let shadow = Mask::new(&pieces, |v| v <= 0.25)
            .clip(&[vec![p(-1., 5.), p(11., 5.)]], &|| false)
            .unwrap();
        assert_eq!(shadow.len(), 1);
        assert!(shadow[0][0].dist(p(0., 5.)) < 1e-9);
        assert!(shadow[0].last().unwrap().dist(p(2.5, 5.)) < 1e-9);
    }
    #[test]
    fn tonal_intervals_split_at_exact_boundaries_without_losing_or_overprinting_light() {
        let regions = vec![
            rectangle(0., 4., 0.),
            rectangle(4., 7., 0.4),
            rectangle(7., 10., 0.9),
        ];
        let lines = vec![vec![p(-1., 5.), p(11., 5.)]];
        let shadow = Mask::new(&regions, |v| accepts(TonalMask::Shadow, v, 0.14))
            .clip(&lines, &|| false)
            .unwrap();
        let light = Mask::new(&regions, |v| accepts(TonalMask::Light, v, 0.14))
            .clip(&lines, &|| false)
            .unwrap();
        assert_eq!(shadow, vec![vec![p(0., 5.), p(4., 5.)]]);
        assert_eq!(light, vec![vec![p(4., 5.), p(10., 5.)]]);
        let band = Mask::new(&regions, |v| {
            accepts(TonalMask::Band { min: 0.2, max: 0.6 }, v, 0.14)
        })
        .clip(&lines, &|| false)
        .unwrap();
        assert_eq!(band, vec![vec![p(4., 5.), p(7., 5.)]]);
        let mut reversed = regions.clone();
        for r in &mut reversed {
            r.polygon.reverse();
        }
        assert_eq!(
            Mask::new(&reversed, |_| true)
                .clip(&lines, &|| false)
                .unwrap(),
            vec![vec![p(0., 5.), p(10., 5.)]]
        );
    }
    #[test]
    fn overlapping_triangles_merge_and_a_torus_hole_is_not_filled() {
        let regions = vec![
            rectangle(0., 4., 0.),
            rectangle(2., 4., 0.),
            rectangle(6., 10., 0.),
        ];
        let out = Mask::new(&regions, |_| true)
            .clip(&[vec![p(-1., 5.), p(11., 5.)]], &|| false)
            .unwrap();
        assert_eq!(
            out,
            vec![vec![p(0., 5.), p(4., 5.)], vec![p(6., 5.), p(10., 5.)]]
        );
    }
    #[test]
    fn dash_phase_crosses_vertices_in_physical_mm_and_cancels() {
        let lines = vec![vec![p(0., 0.), p(1., 0.), p(1., 5.)]];
        let output = dashed(
            &lines,
            DashPattern {
                dash_mm: 2.,
                gap_mm: 1.,
            },
            &|| false,
        )
        .unwrap();
        assert_eq!(
            output,
            vec![
                vec![p(0., 0.), p(1., 0.), p(1., 1.)],
                vec![p(1., 2.), p(1., 4.)]
            ]
        );
        assert!(dashed(&lines, DashPattern::default(), &|| true).is_err());
    }
    fn drawing(mesh: Mesh) -> (Drawing, Settings) {
        let settings = Settings {
            recipe: RecipeKind::XRay,
            ..Default::default()
        };
        let drawing = Engine::default()
            .draw(Arc::new(mesh), &settings, &crate::Paper::default(), &|| {
                false
            })
            .unwrap();
        (drawing, settings)
    }
    #[test]
    fn projected_light_uses_corrected_normals_camera_occlusion_and_cast_shadows() {
        let (drawing, settings) = drawing(Primitive::Torus.mesh());
        let mut cache = ToneCache::default();
        let regions = cache.regions(&drawing, &settings, &|| false).unwrap();
        assert!(regions.iter().any(|r| r.value == 0.));
        assert!(regions.iter().any(|r| r.value > 0.6));
        // El toro es liso: el tono varía dentro de las caras y nunca sale de [0, 1].
        assert!(regions.iter().any(|r| {
            let lo = r.values.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = r.values.iter().copied().fold(0., f64::max);
            hi - lo > 1e-3
        }));
        assert!(regions
            .iter()
            .flat_map(|r| r.values.iter().chain([&r.value]))
            .all(|v| (0. ..=1.).contains(v)));
        for region in regions.iter().step_by(19) {
            let center = region.polygon.iter().fold(p(0., 0.), |sum, q| sum + *q)
                * (1. / region.polygon.len() as f64);
            assert!(center.x.is_finite() && center.y.is_finite());
        }
        let again = cache.regions(&drawing, &settings, &|| false).unwrap();
        assert!(Arc::ptr_eq(&regions, &again));
        let mut turned = settings.clone();
        turned.shadow.azimuth += 90.;
        assert!(!Arc::ptr_eq(
            &regions,
            &cache.regions(&drawing, &turned, &|| false).unwrap()
        ));
    }
    #[test]
    fn crosshatch_adds_families_only_to_darker_bands_with_global_mm_phase() {
        let (drawing, settings) = drawing(Primitive::Cube.mesh());
        let mut cache = ToneCache::default();
        // Campo controlado para probar las cuatro densidades, sin depender de una primitiva.
        let regions = Arc::new(vec![
            rectangle(20., 60., 0.1),
            rectangle(60., 100., 0.3),
            rectangle(100., 140., 0.5),
            rectangle(140., 180., 0.7),
        ]);
        cache.entries.push(Entry {
            mesh: drawing.mesh.clone(),
            camera: drawing.camera,
            rotation: settings.paper_rotation,
            light: (settings.shadow.azimuth, settings.shadow.elevation),
            regions,
        });
        let params = ToneHatchParams {
            spacing_mm: 1.,
            ..Default::default()
        };
        let paper = crate::Paper {
            margin_mm: 0.,
            ..Default::default()
        };
        let output = cache
            .hatch(&drawing, &params, &settings, &paper, &|| false)
            .unwrap();
        let mut angles = std::collections::BTreeMap::<i32, std::collections::BTreeSet<i32>>::new();
        for line in output {
            let d = line[1] - line[0];
            let angle = d.y.atan2(d.x).to_degrees().rem_euclid(180.).round() as i32;
            for band in 0..4 {
                if polygon_interval(
                    line[0],
                    line[1],
                    &rectangle(20.01 + 40. * band as f64, 59.99 + 40. * band as f64, 0.).polygon,
                )
                .is_some()
                {
                    angles.entry(band).or_default().insert(angle);
                }
            }
        }
        assert_eq!(angles.get(&0).unwrap().len(), 4);
        assert_eq!(angles.get(&1).unwrap().len(), 3);
        assert_eq!(angles.get(&2).unwrap().len(), 2);
        assert_eq!(angles.get(&3).unwrap().len(), 1);
    }
}

#[cfg(test)]
mod cast_tests {
    use super::*;
    #[test]
    fn a_lit_receiver_is_cut_to_zero_where_another_component_blocks_the_sun() {
        let mesh = Mesh {
            name: "Dos placas".into(),
            vertices: vec![
                [-2., -2., 0.],
                [2., -2., 0.],
                [2., 2., 0.],
                [-2., 2., 0.],
                [-0.5, -0.5, 1.],
                [0.5, -0.5, 1.],
                [0.5, 0.5, 1.],
                [-0.5, 0.5, 1.],
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]],
            objects: vec![],
            face_objects: vec![],
        };
        let settings = Settings {
            recipe: RecipeKind::XRay,
            yaw: 0.,
            elevation: 89.99,
            shadow: ShadowSettings {
                azimuth: 0.,
                elevation: 45.,
                ..Default::default()
            },
            ..Default::default()
        };
        let drawing = Engine::default()
            .draw(Arc::new(mesh), &settings, &crate::Paper::default(), &|| {
                false
            })
            .unwrap();
        let mut cache = ToneCache::default();
        let regions = cache.regions(&drawing, &settings, &|| false).unwrap();
        // La luz cae hacia -X: la placa superior tapa el sol a la izquierda,
        // pero la cámara cenital sigue viendo esa parte de la placa inferior.
        let shadow = drawing.camera.project(V3::new(-0.5, 0., -0.25)).0;
        let lit = drawing.camera.project(V3::new(0.5, 0., -0.25)).0;
        let value_at = |point| {
            regions
                .iter()
                .filter(|r| crate::geom::inside_rings(point, std::slice::from_ref(&r.polygon)))
                .map(|r| r.value)
                .fold(1f64, f64::min)
        };
        assert_eq!(value_at(shadow), 0.);
        assert!((value_at(lit) - 45f64.to_radians().sin()).abs() < 1e-8);
        // La placa que tapa a la cámara no deja asomar la sombra de la placa de abajo.
        let center = drawing.camera.project(V3::new(0., 0., 0.25)).0;
        assert!(value_at(center) > 0.6);
    }
}
