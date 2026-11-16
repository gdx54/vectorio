use super::*;
pub(super) fn program(s: &mut Settings, p: &ShadowSettings) -> usize {
    s.recipe = RecipeKind::FlatShadow;
    s.shadow = p.clone();
    s.shadow.ground = true;
    // Se utiliza la misma máscara proyectada y la misma fase de trama.
    s.shadow.outline = false;
    3
}
