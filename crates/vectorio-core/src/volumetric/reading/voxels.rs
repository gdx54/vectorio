//! Volumen discretizado por centros de celda. Las tres familias corresponden
//! a las normales X, Y y Z, incluso al orbitar o cambiar las tintas.
use super::*;

fn coords(axis: usize, depth: usize, u: usize, v: usize) -> [usize; 3] {
    let mut q = [0; 3];
    q[axis] = depth;
    q[(axis + 1) % 3] = u;
    q[(axis + 2) % 3] = v;
    q
}

struct Grid {
    size: [usize; 3],
    origin: V3,
    step: f64,
    cells: Vec<bool>,
}

impl Grid {
    fn index(&self, [x, y, z]: [usize; 3]) -> usize {
        (z * self.size[1] + y) * self.size[0] + x
    }

    fn corner(&self, q: [usize; 3]) -> V3 {
        self.origin + v3(q.map(|i| i as f64)) * self.step
    }

    fn sample(
        mesh: &Mesh,
        geometry: &fields::Geometry,
        divisions: usize,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Self, String> {
        let [lo, hi] = mesh.bounds();
        let span = arr(hi - lo);
        let step = span.into_iter().fold(0., f64::max) / divisions as f64;
        let size = span.map(|v| (v / step).ceil().clamp(1., divisions as f64) as usize);
        let origin = (lo + hi - v3(size.map(|n| n as f64)) * step) * 0.5;
        let mut grid = Grid {
            size,
            origin,
            step,
            cells: vec![false; size.iter().product()],
        };
        let rays = geometry.rays(mesh, cancel)?;
        // Los sólidos se llenan por paridad. En una malla abierta muestreamos
        // la piel desde los tres ejes, también si la lámina es vertical.
        let axes = if geometry.closed { 2..3 } else { 0..3 };
        let mut hits = Vec::new();
        for axis in axes {
            let a = (axis + 1) % 3;
            let b = (axis + 2) % 3;
            let mut dir = [0.; 3];
            dir[axis] = 1.;
            for v in 0..size[b] {
                for u in 0..size[a] {
                    if cancel() {
                        return Err("Cálculo cancelado.".into());
                    }
                    let mut start = arr(origin);
                    // Evita lanzar por una arista compartida entre triángulos.
                    start[a] += (u as f64 + 0.500001) * step;
                    start[b] += (v as f64 + 0.500003) * step;
                    let mut depth = -step;
                    let limit = size[axis] as f64 * step;
                    hits.clear();
                    while depth < limit {
                        start[axis] = arr(origin)[axis] + depth;
                        let Some(hit) =
                            rays.nearest(v3(start), v3(dir), &[], limit - depth + step * 1e-5)
                        else {
                            break;
                        };
                        let crossing = depth + hit;
                        hits.push(crossing / step);
                        depth = crossing + step * 1e-5;
                    }
                    let mut mark = |k| {
                        let i = grid.index(coords(axis, k, u, v));
                        grid.cells[i] = true;
                    };
                    if geometry.closed && hits.len() % 2 == 0 {
                        for interval in hits.chunks_exact(2) {
                            let first = (interval[0] - 0.5).ceil().max(0.) as usize;
                            let end = (interval[1] - 0.5).ceil().min(size[axis] as f64) as usize;
                            for k in first..end {
                                mark(k);
                            }
                        }
                    } else {
                        for hit in &hits {
                            mark((hit.floor() as usize).min(size[axis] - 1));
                        }
                    }
                }
            }
        }
        Ok(grid)
    }
}

pub(super) fn draw(
    d: &Drawing,
    s: &Settings,
    geometry: &fields::Geometry,
    divisions: usize,
    hatch: bool,
    spacing_mm: f64,
    paper: &crate::Paper,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Polyline>>, String> {
    let grid = Grid::sample(&d.mesh, geometry, divisions, cancel)?;
    let mut scene = crate::scene::Scene::new(d.camera, effects::area(paper));
    scene.set_intersections(false);
    let axis_view = d.camera.axes().2;
    let (sin, cos) = s.paper_rotation.to_radians().sin_cos();
    let rotate = |q: V3| {
        let r = q - d.camera.world_center;
        d.camera.world_center
            + r * cos
            + axis_view.cross(r) * sin
            + axis_view * (axis_view.dot(r) * (1. - cos))
    };
    let parts = ["X", "Y", "Z"].map(|name| scene.part(name));
    for (axis, part) in parts.into_iter().enumerate() {
        scene.set_part(part);
        let width = grid.size[(axis + 1) % 3];
        let height = grid.size[(axis + 2) % 3];
        let mut exposed = vec![false; width * height];
        for face in 0..=grid.size[axis] {
            if cancel() {
                return Err("Cálculo cancelado.".into());
            }
            let point = grid.corner(coords(axis, face, 0, 0));
            let normal = v3(coords(axis, 1, 0, 0).map(|x| x as f64));
            let positive = d.camera.facing_at(normal, point);
            if !positive && !d.camera.facing_at(normal * -1., point) {
                continue;
            }
            for v in 0..height {
                for u in 0..width {
                    let below = face > 0 && grid.cells[grid.index(coords(axis, face - 1, u, v))];
                    let above =
                        face < grid.size[axis] && grid.cells[grid.index(coords(axis, face, u, v))];
                    exposed[v * width + u] = below != above && below == positive;
                }
            }
            // Agrupar caras coplanarias alarga los trazos y evita levantar la pluma en cada celda.
            let corner = |u, v| rotate(grid.corner(coords(axis, face, u, v)));
            for v in 0..height {
                for u in 0..width {
                    if !exposed[v * width + u] {
                        continue;
                    }
                    let mut end = u + 1;
                    while end < width && exposed[v * width + end] {
                        end += 1;
                    }
                    let mut top = v + 1;
                    while top < height
                        && exposed[top * width + u..top * width + end]
                            .iter()
                            .all(|x| *x)
                    {
                        top += 1;
                    }
                    for row in v..top {
                        exposed[row * width + u..row * width + end].fill(false);
                    }
                    let quad = [
                        corner(u, v),
                        corner(end, v),
                        corner(end, top),
                        corner(u, top),
                    ];
                    scene.occluder(&quad);
                    for col in u..=end {
                        scene.line(&[corner(col, v), corner(col, top)]);
                    }
                    for row in v..=top {
                        scene.line(&[corner(u, row), corner(end, row)]);
                    }
                    if hatch {
                        let edge = d.camera.project(quad[1]).0 - d.camera.project(quad[0]).0;
                        scene.hatch_surface(
                            &[quad.to_vec()],
                            edge.y.atan2(edge.x).to_degrees(),
                            spacing_mm,
                        );
                    }
                }
            }
        }
    }
    let rendered = scene
        .render_cancellable(cancel)
        .ok_or("Cálculo cancelado.")?;
    Ok(rendered
        .parts
        .into_iter()
        .skip(1)
        .map(|part| part.lines)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(mesh: Mesh, divisions: usize) -> Grid {
        let mesh = Arc::new(mesh);
        let geometry = fields::Cache::default()
            .geometry(mesh.clone(), &|| false)
            .unwrap();
        Grid::sample(&mesh, &geometry, divisions, &|| false).unwrap()
    }

    #[test]
    fn voxel_grid_preserves_a_torus_hole_and_open_vertical_sheet() {
        let grid = sample(Primitive::Torus.mesh(), 21);
        let center = grid.size.map(|s| s / 2);
        for z in 0..grid.size[2] {
            assert!(!grid.cells[grid.index([center[0], center[1], z])]);
        }
        assert!(grid.cells[grid.index([center[0] + 7, center[1], center[2]])]);
        let mut plane = Primitive::Plane.mesh();
        for vertex in &mut plane.vertices {
            vertex.swap(0, 2);
        }
        let grid = sample(plane, 9);
        assert_eq!(grid.size, [1, 9, 9]);
        assert!(grid.cells.iter().all(|cell| *cell));
    }

    #[test]
    fn voxel_cube_only_draws_grids_on_the_three_visible_faces() {
        let mesh = Arc::new(Primitive::Cube.mesh());
        let scene = Settings::default();
        let mut passes = ReadingStyle::VoxelGrid.passes(&scene);
        let params = match &mut passes[0].kind {
            PassKind::Reading(params) => params,
            _ => unreachable!(),
        };
        params.effect = ReadingEffect::Voxels {
            divisions: 8,
            hatch: false,
            spacing_mm: 0.4,
        };
        let mut engine = Engine::default();
        let paper = crate::Paper::default();
        let draw = engine
            .draw_passes(mesh.clone(), &scene, &passes, &paper, &|| false)
            .unwrap();
        for family in &draw.parts[0].families {
            assert_eq!(family.lines.len(), 2 * (8 + 1));
        }
        passes[0].reading.as_mut().unwrap().pens = vec![1, 4, 2];
        let next = engine
            .draw_passes(mesh, &scene, &passes, &paper, &|| false)
            .unwrap();
        for (i, (old, new)) in draw.parts[0]
            .families
            .iter()
            .zip(&next.parts[0].families)
            .enumerate()
        {
            assert_eq!(old.lines, new.lines);
            assert_eq!(new.pen, [1, 4, 2][i]);
        }
    }
}
