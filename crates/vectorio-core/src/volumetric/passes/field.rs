use super::*;
pub(super) fn program(s: &mut Settings, p: &SectionParams) -> usize {
    s.recipe = RecipeKind::Atlas;
    sections::parameters(s, p)
}
