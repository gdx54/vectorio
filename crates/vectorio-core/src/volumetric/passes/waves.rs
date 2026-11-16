use super::*;
pub(super) fn program(s: &mut Settings, p: &WaveParams) -> usize {
    s.recipe = if p.family == CurveFamily::All {
        RecipeKind::Hybrid
    } else {
        RecipeKind::SurfaceWaves
    };
    s.major_every = p.major_every;
    // Solo se selecciona la familia de ondas del programa combinado.
    s.art = ArtSettings {
        facets: p.facets,
        lines: p.lines,
        pattern: p.pattern,
        origin_azimuth: p.origin_azimuth,
        origin_elevation: p.origin_elevation,
        deformation: p.deformation,
        frequency: p.frequency,
        outline: p.outline,
        mesh_inks: 2,
        ..Default::default()
    };
    match p.family {
        CurveFamily::Secondary => 1,
        _ => 0,
    }
}
