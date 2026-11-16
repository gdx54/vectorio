use super::*;
pub(super) fn program(s: &mut Settings, p: &MeshParams) -> usize {
    s.recipe = RecipeKind::TriMesh;
    s.art.facets = p.facets;
    s.art.coloring = p.coloring;
    s.art.mesh_inks = p.inks;
    p.group
}
