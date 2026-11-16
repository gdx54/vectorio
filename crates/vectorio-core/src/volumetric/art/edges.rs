//! Todas las aristas únicas de triangulación, agrupadas sin depender del winding.
use super::*;
use std::collections::BTreeMap;
pub(super) fn strokes(
    mesh: &Mesh,
    s: &ArtSettings,
    camera: &Camera,
    hybrid: bool,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<MeshStroke>, String> {
    let points = mesh.points();
    let mut edges = BTreeMap::<(usize, usize), (V3, usize)>::new();
    for (i, t) in mesh.triangles.iter().enumerate() {
        if i % 256 == 0 && cancel() {
            return Err(CANCEL.into());
        }
        let normal = (points[t[1]] - points[t[0]])
            .cross(points[t[2]] - points[t[0]])
            .normalized();
        let n = V3::new(normal.x.abs(), normal.y.abs(), normal.z.abs());
        let owner = mesh.face_objects.get(i).copied().unwrap_or(0);
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let value = edges
                .entry((a.min(b), a.max(b)))
                .or_insert((V3::default(), owner));
            value.0 = value.0 + n;
            value.1 = value.1.min(owner);
        }
    }
    let count = s.mesh_inks;
    let metric = |q: V3| {
        if s.coloring == MeshColor::Depth {
            camera.project(q).1
        } else {
            q.z
        }
    };
    let (lo, hi) = points
        .iter()
        .map(|q| metric(*q))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
            (a.min(v), b.max(v))
        });
    let mut result = Vec::with_capacity(edges.len());
    for (i, ((a, b), (n, owner))) in edges.into_iter().enumerate() {
        if i % 256 == 0 && cancel() {
            return Err(CANCEL.into());
        }
        let class = match s.coloring {
            MeshColor::Orientation => {
                if count == 2 {
                    usize::from(n.z < n.x.max(n.y))
                } else if n.z >= n.x && n.z >= n.y {
                    0
                } else if n.y >= n.x {
                    1
                } else {
                    2
                }
            }
            MeshColor::Objects => owner % count,
            MeshColor::Height | MeshColor::Depth => (((metric((points[a] + points[b]) * 0.5) - lo)
                / (hi - lo).max(1e-12)
                * count as f64)
                .floor() as usize)
                .min(count - 1),
        };
        result.push(MeshStroke {
            points: [points[a], points[b]],
            role: if hybrid {
                1 + class % count
            } else {
                class % count
            },
        });
    }
    Ok(result)
}
