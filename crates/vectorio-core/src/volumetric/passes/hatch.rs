use super::*;
pub(super) fn program(s: &mut Settings, p: &ShadowSettings, middle: bool) -> usize {
    s.recipe = RecipeKind::FlatShadow;
    s.shadow = p.clone();
    usize::from(middle)
}
