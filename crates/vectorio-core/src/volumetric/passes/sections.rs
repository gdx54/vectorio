use super::*;
pub(super) fn program(s: &mut Settings, p: &SectionParams) -> usize {
    s.recipe = RecipeKind::Strata;
    s.strata_shift_mm = p.shift_mm;
    parameters(s, p)
}
pub(super) fn parameters(s: &mut Settings, p: &SectionParams) -> usize {
    s.field = p.field;
    s.direction = p.direction;
    s.cuts = p.cuts;
    s.spacing = p.spacing;
    s.offset = p.offset;
    s.range = p.range;
    s.major_every = if p.family == CurveFamily::All {
        1
    } else {
        p.major_every
    };
    match p.family {
        CurveFamily::Secondary => 1,
        _ => 0,
    }
}
