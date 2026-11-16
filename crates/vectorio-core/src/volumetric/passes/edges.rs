use super::*;
pub(super) fn program(s: &mut Settings, angle: f64) -> usize {
    s.recipe = RecipeKind::XRay;
    s.edge_angle = angle;
    2
}
