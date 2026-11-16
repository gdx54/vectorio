//! Malla de trabajo por colapso de aristas con error acotado. El original no se toca.
use super::*;
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeshDetail {
    Interactive,
    Detailed,
    Original,
}
impl MeshDetail {
    pub fn target(self) -> usize {
        match self {
            Self::Interactive => 40_000,
            Self::Detailed => 160_000,
            Self::Original => usize::MAX,
        }
    }
    pub fn error(self) -> f32 {
        match self {
            Self::Interactive => 0.001,
            Self::Detailed => 0.0001,
            Self::Original => 0.0,
        }
    }
}
#[derive(Clone, Copy)]
struct Position([f32; 3]);
impl meshopt::DecodePosition for Position {
    fn decode_position(&self) -> [f32; 3] {
        self.0
    }
}
/// El objetivo no se fuerza destruyendo componentes: si la topología o el error
/// impiden llegar, se conservan más caras. Cada objeto se reduce por separado.
pub(super) fn reduced(
    mesh: &Mesh,
    target: usize,
    error: f32,
    cancel: &dyn Fn() -> bool,
) -> Result<Mesh, String> {
    if mesh.triangles.len() <= target {
        return Ok(mesh.clone());
    }
    let [lo, hi] = mesh.bounds();
    let center = (lo + hi) * 0.5;
    let extent = (hi - lo).length().max(1e-12);
    let positions: Vec<_> = mesh
        .vertices
        .iter()
        .map(|p| {
            let p = (v3(*p) - center) * (1. / extent);
            Position([p.x as f32, p.y as f32, p.z as f32])
        })
        .collect();
    let mut groups = BTreeMap::<usize, Vec<u32>>::new();
    for (i, t) in mesh.triangles.iter().enumerate() {
        if i % 4096 == 0 && cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let owner = mesh.face_objects.get(i).copied().unwrap_or(0);
        groups.entry(owner).or_default().extend(t.map(|i| i as u32));
    }
    let mut triangles = Vec::new();
    let mut owners = Vec::new();
    for (owner, indices) in groups {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let goal = ((target as f64 * indices.len() as f64 / (mesh.triangles.len() * 3) as f64)
            .ceil() as usize)
            .max(4)
            * 3;
        let indices = meshopt::simplify_decoder(
            &indices,
            &positions,
            goal.min(indices.len()),
            error,
            meshopt::SimplifyOptions::None,
            None,
        );
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        for t in indices.chunks_exact(3) {
            triangles.push([t[0] as usize, t[1] as usize, t[2] as usize]);
            if !mesh.face_objects.is_empty() {
                owners.push(owner);
            }
        }
    }
    // Compactar sin cuantizar ni cambiar los vértices originales elegidos.
    let mut remap = vec![usize::MAX; mesh.vertices.len()];
    let mut vertices = Vec::new();
    for t in &mut triangles {
        for id in t {
            let next = &mut remap[*id];
            if *next == usize::MAX {
                *next = vertices.len();
                vertices.push(mesh.vertices[*id]);
            }
            *id = *next;
        }
    }
    Ok(Mesh {
        name: mesh.name.clone(),
        vertices,
        triangles,
        objects: mesh.objects.clone(),
        face_objects: owners,
    })
}
