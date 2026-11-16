use super::*;
use crate::p;
pub struct Spiro;
fn gcd(mut a: usize, mut b: usize) -> usize { while b != 0 { (a, b) = (b, a % b); } a }
impl Generator for Spiro {
    fn id(&self) -> &'static str { "spiro" }
    fn name(&self) -> &'static str { "Espirógrafo" }
    fn category(&self) -> &'static str { "Curvas" }
    fn inks(&self) -> &'static super::InkPlan { &super::inks::SPIRO }
    fn description(&self) -> &'static str { "Curvas cerradas de engranajes, una pasada por capa." }
    fn params(&self) -> Vec<ParamSpec> { vec![
        ParamSpec::Section { label: "Engranajes" },
        choice("kind", "Curva", &[("hypo", "Hipotrocoide"), ("epi", "Epitrocoide")], "hypo"),
        range("outer", "Radio fijo", 3.0, 18.0, 1.0, 9.0), range("inner", "Radio móvil", 1.0, 12.0, 1.0, 4.0),
        range("distance", "Distancia del lápiz", 0.1, 1.5, 0.05, 0.8),
        ParamSpec::Section { label: "Capas" }, range("layers", "Número de capas", 1.0, 6.0, 1.0, 3.0),
        ParamSpec::Range { id: "phase", label: "Desfase", min: 0.0, max: 90.0, step: 1.0, default: 12.0,
            random: Some((5.0, 45.0)), show: Some(Show { id: "layers", equals: "!1" }), hint: Some("Giro adicional de cada capa.") },
    ] }
    fn randomize(&self, rng: &mut Rng, params: &mut Params, locks: &BTreeSet<String>) {
        randomize_specs(&self.params(), rng, params, locks);
        if params.choice("kind") == "hypo" && params.int("outer") == params.int("inner") {
            if !locks.contains("inner") { params.set("inner", ParamValue::Number((params.int("inner") - 1).max(1) as f64)); }
            else if !locks.contains("outer") { params.set("outer", ParamValue::Number((params.int("outer") + 1).min(18) as f64)); }
        }
    }
    fn generate(&self, params: &Params, ctx: &GenContext) -> Result<GenOutput, String> {
        if !ctx.valid() { return Err("Área o rotulador no válido".into()); }
        let prm = params.normalized(&self.params());
        let (outer, inner) = (prm.int("outer"), prm.int("inner"));
        let (r, small) = (outer as f64, inner as f64);
        let epi = prm.choice("kind") == "epi";
        let base = if epi { r + small } else { r - small };
        // Con radios iguales, una hipotrocoide es un punto y no un trazo para plotter.
        if base.abs() < 1e-9 { return Err("Usa radios distintos para la hipotrocoide.".into()); }
        let distance = prm.f64("distance") * small;
        let radius = base.abs() + distance;
        let scale = ctx.area.width().min(ctx.area.height()) * 0.47 / radius;
        let period = std::f64::consts::TAU * inner as f64 / gcd(outer, inner) as f64;
        let samples = ((period * (base.abs() + distance * (base / small).abs()) * scale / 0.25).ceil() as usize)
            .clamp(512, if ctx.detail == Detail::Preview { 12000 } else { 40000 });
        let mut parts = Vec::new();
        let mut rng = Rng::new(ctx.seed); let start = rng.f() * std::f64::consts::TAU;
        for layer in 0..prm.int("layers") {
            let phase = start + (layer as f64 * prm.f64("phase")).to_radians();
            let mut line: Polyline = (0..samples).map(|i| {
                let t = period * i as f64 / samples as f64;
                let sign = if epi { -1.0 } else { 1.0 };
                let q = p(base * t.cos() + sign * distance * (base / small * t).cos(),
                    base * t.sin() - distance * (base / small * t).sin());
                ctx.area.center() + q.rotate(phase) * scale
            }).collect();
            line.push(line[0]);
            parts.push(GenPart { name: format!("Capa {}", layer + 1), lines: vec![line], pen_order: None });
        }
        Ok(GenOutput { parts, warning: None })
    }
}
