//! Adaptadores de los algoritmos existentes. No alteran sus tolerancias ni el orden de trazos.
mod edges;
mod field;
mod ground;
mod hatch;
mod mesh;
mod outline;
mod sections;
mod waves;
use super::*;

/// Contexto compartido de escena. La malla, campos y oclusores siguen cacheados
/// por Pipeline; el motor cachea además la salida proyectada de cada programa.
pub(super) struct PassCtx<'a> {
    pub scene: &'a Settings,
}
pub(super) fn program(scene: &Settings, pass: &Pass) -> (Settings, usize) {
    let ctx = PassCtx { scene };
    let mut s = Settings {
        mesh_detail: ctx.scene.mesh_detail,
        projection: ctx.scene.projection.clone(),
        yaw: ctx.scene.yaw,
        elevation: ctx.scene.elevation,
        model_rotation: ctx.scene.model_rotation,
        model_scale: ctx.scene.model_scale,
        simplify_mm: ctx.scene.simplify_mm,
        zoom: ctx.scene.zoom,
        position_mm: ctx.scene.position_mm,
        paper_rotation: ctx.scene.paper_rotation,
        seed: ctx.scene.seed,
        visible_only: pass.visibility != Visibility::All,
        hidden_lines: pass.visibility == Visibility::Hidden,
        halo_mm: pass.finish.halo_mm,
        ..Default::default()
    };
    let role = match &pass.kind {
        PassKind::Outline(p) => outline::program(&mut s, p),
        PassKind::Edges { angle } => edges::program(&mut s, *angle),
        PassKind::Sections(p) => sections::program(&mut s, p),
        PassKind::Field(p) => field::program(&mut s, p),
        PassKind::Waves(p) => waves::program(&mut s, p),
        PassKind::Mesh(p) => mesh::program(&mut s, p),
        PassKind::Hatch { settings, middle } => hatch::program(&mut s, settings, *middle),
        PassKind::Ground(p) => ground::program(&mut s, p),
        PassKind::Reading(p) => {
            s.recipe = RecipeKind::TriMesh;
            s.art.facets = p.facets;
            s.art.mesh_inks = 1;
            0
        }
        PassKind::ToneHatch(p) => {
            s.recipe = RecipeKind::Atlas;
            s.cuts = p.cuts;
            s.major_every = 1;
            // Normal inversa transpuesta: los planos pertenecen al objeto,
            // incluso con escala XYZ no uniforme y rotación del modelo.
            let mut n = V3::new(
                p.direction[0] / scene.model_scale[0],
                p.direction[1] / scene.model_scale[1],
                p.direction[2] / scene.model_scale[2],
            );
            let (sin, cos) = scene.model_rotation[0].to_radians().sin_cos();
            n = V3::new(n.x, cos * n.y - sin * n.z, sin * n.y + cos * n.z);
            let (sin, cos) = scene.model_rotation[1].to_radians().sin_cos();
            n = V3::new(cos * n.x + sin * n.z, n.y, -sin * n.x + cos * n.z);
            let (sin, cos) = scene.model_rotation[2].to_radians().sin_cos();
            n = V3::new(cos * n.x - sin * n.y, sin * n.x + cos * n.y, n.z);
            s.direction = arr(n.normalized());
            0
        }
    };
    (s, role)
}
