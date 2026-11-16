//! Métrica de densidad de una lámina: cuánta gente, cuántos metros de trazo y
//! cuántos metros por decímetro cuadrado de papel. Sirve para comparar semillas y
//! para vigilar que una escena densa no se pase de lo que el rotulador aguanta.
use crate::BBox;
use super::super::GenOutput;

/// Resumen de densidad de una salida.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Density {
    pub people: usize,
    pub meters: f64,
    pub per_dm2: f64,
}

/// Densidad de `output` sobre el área `area` (mm), con `people` figuras dibujadas.
pub fn density(output: &GenOutput, area: BBox, people: usize) -> Density {
    let meters = output.total_m();
    let width = area.max.x - area.min.x;
    let height = area.max.y - area.min.y;
    let dm2 = width * height / 10_000.0;
    let per_dm2 = if dm2.is_finite() && dm2 > 0.0 { meters / dm2 } else { 0.0 };
    Density { people, meters, per_dm2 }
}
