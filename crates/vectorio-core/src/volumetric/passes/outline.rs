use super::*;
pub(super) fn program(s: &mut Settings, p: &OutlineParams) -> usize {
    match p {
        OutlineParams::Surface { facets } => {
            s.recipe = RecipeKind::SurfaceWaves;
            s.art.facets = *facets;
            s.art.outline = true;
        }
        OutlineParams::Shadow(p) => {
            s.recipe = RecipeKind::FlatShadow;
            s.shadow = p.clone();
            s.shadow.outline = true;
        }
    }
    2
}
