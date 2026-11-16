//! Mundos de cúpulas: arquitectura pequeña dentro de grandes siluetas conectadas.
use crate::{p, geom::Rng, scene::{Camera, Scene, V3}};
use super::{choice, inks, range, Detail, GenContext, GenOutput, Generator, InkPlan, Params, ParamSpec, Show};
use super::kit::{self, v, Crowd, Dome};
use super::style::{part, Style};

mod habitats;
mod surroundings;
mod expeditions;
#[cfg(test)] mod tests;

pub struct Domes;

/// Posiciones en la hoja: jerarquía de tamaños y huecos para pasos y criatura.
const PLACES: [(f64, f64, f64); 9] = [
    (0.50, 0.53, 0.17), (0.23, 0.29, 0.13), (0.79, 0.26, 0.115),
    (0.20, 0.75, 0.125), (0.79, 0.78, 0.12), (0.51, 0.15, 0.08),
    (0.51, 0.91, 0.075), (0.09, 0.51, 0.07), (0.91, 0.52, 0.07),
];

#[derive(Clone, Copy)]
struct Pod {
    center: V3,
    radius: f64,
    habitat: usize,
    seed: u64,
}

struct World<'a> {
    prm: &'a Params,
    ctx: &'a GenContext,
    style: Style,
    space: bool,
}

impl World<'_> {
    fn camera(&self) -> Camera {
        Camera {
            yaw_deg: self.prm.choice("yaw").parse().unwrap_or(225.0),
            scale: (self.ctx.area.width().min(self.ctx.area.height()) / 75.0).clamp(1.6, 4.0),
            world_center: V3::default(),
            paper_center: self.ctx.area.center(),
            ..Default::default()
        }
    }

    fn paper_point(&self, x: f64, y: f64) -> crate::geom::P {
        p(self.ctx.area.min.x + x * self.ctx.area.width(), self.ctx.area.min.y + y * self.ctx.area.height())
    }

    fn pods(&self, camera: &Camera) -> Vec<Pod> {
        let mut rng = Rng::new(self.ctx.seed);
        let mut themes = [0, 1, 2, 3, 4, 5];
        for k in 1..themes.len() {
            let next = k + (rng.f() * (themes.len() - k) as f64) as usize;
            themes.swap(k, next.min(5));
        }
        let short = self.ctx.area.width().min(self.ctx.area.height());
        PLACES.iter().take(self.prm.int("domes")).enumerate().map(|(i, &(x, y, r))| {
            let radius = r * short / camera.scale * (0.94 + rng.f() * 0.06);
            let mut point = self.paper_point(x, y);
            point.y += radius * camera.scale * (1.0 - camera.elev_deg.to_radians().sin()) * 0.5;
            let center = camera.ground(point, 3.0 + (i % 3) as f64 * 4.0);
            Pod { center, radius, habitat: themes[i % themes.len()], seed: rng.next_u64() }
        }).collect()
    }

    fn scene(&self) -> Scene {
        let camera = self.camera();
        let pods = self.pods(&camera);
        let mut scene = Scene::new(camera, self.ctx.area);
        self.style.prepare(&mut scene);
        scene.set_sun(Some(v(1.0, -0.6, 1.4)));
        self.background(&mut scene);
        for (a, b) in links(&pods) {
            tunnel(&mut scene, pods[a], pods[b]);
        }
        let mut crowd = Crowd::new(self.style);
        for pod in &pods {
            let dome = Dome { center: pod.center, radius: pod.radius, plinth: 0.8, ribs: if self.ctx.detail == Detail::Preview { 6 } else { 8 } };
            let mut rng = Rng::new(pod.seed);
            kit::dome(&mut scene, dome, |scene, floor| self.habitat(scene, &mut crowd, &mut rng, floor, pod.habitat));
        }
        self.protagonist(&mut scene);
        scene
    }
}

/// Árbol de proximidad: todas las cúpulas quedan conectadas sin cruces innecesarios.
fn links(pods: &[Pod]) -> Vec<(usize, usize)> {
    let mut visited = vec![false; pods.len()];
    let mut edges = Vec::new();
    if pods.is_empty() { return edges; }
    visited[0] = true;
    while edges.len() + 1 < pods.len() {
        let next = (0..pods.len()).filter(|a| visited[*a]).flat_map(|a| {
            (0..pods.len()).filter(|b| !visited[*b]).map(move |b| (a, b))
        }).min_by(|(a, b), (c, d)| {
            (pods[*b].center - pods[*a].center).length().total_cmp(&(pods[*d].center - pods[*c].center).length())
        });
        let Some((a, b)) = next else { break };
        visited[b] = true;
        edges.push((a, b));
    }
    edges
}

/// Pasarela de vidrio: se recorta en las puertas, sin rayas atravesando el interior.
fn tunnel(scene: &mut Scene, a: Pod, b: Pod) {
    let direction = (b.center - a.center).normalized();
    let start = a.center + direction * (a.radius * 0.96) + v(0.0, 0.0, 1.0);
    let end = b.center - direction * (b.radius * 0.96) + v(0.0, 0.0, 1.0);
    if (end - start).dot(direction) < 0.5 { return; }
    let side = direction.cross(v(0.0, 0.0, 1.0)).normalized();
    let up = side.cross(direction).normalized();
    let radius = 0.85;
    part(scene, "contorno");
    for offset in [side, side * -1.0, up * -1.0] {
        scene.line(&[start + offset * radius, end + offset * radius]);
    }
    part(scene, "detalle");
    let rings = ((end - start).length() / 3.0).ceil().max(1.0) as usize;
    for k in 0..=rings {
        let center = start + (end - start) * (k as f64 / rings as f64);
        let ring: Vec<V3> = (0..16).map(|i| {
            let angle = std::f64::consts::TAU * i as f64 / 16.0;
            center + (side * angle.cos() + up * angle.sin()) * radius
        }).collect();
        scene.closed(&ring);
    }
}

impl Generator for Domes {
    fn id(&self) -> &'static str { "domes" }
    fn name(&self) -> &'static str { "Cúpulas" }
    fn category(&self) -> &'static str { "Escenas" }
    fn description(&self) -> &'static str { "Pequeños mundos conectados bajo el mar o entre las estrellas." }
    fn inks(&self) -> &'static InkPlan { &inks::DOMES }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            choice("world", "Mundo", &[("underwater", "Subacuático"), ("space", "Espacial")], "underwater"),
            range("domes", "Número de cúpulas", 5.0, 9.0, 1.0, 7.0),
            range("people", "Gente", 0.0, 1.0, 0.1, 0.6),
            range("background", "Densidad de fondo", 0.0, 1.0, 0.1, 0.6),
            ParamSpec::Choice { id: "creature", label: "Criatura", options: &[("whale", "Ballena"), ("squid", "Calamar"), ("saurian", "Reptil marino"), ("none", "Sin criatura")],
                default: "whale", random: true, show: Some(Show { id: "world", equals: "underwater" }), hint: None },
            ParamSpec::Choice { id: "craft", label: "Protagonista", options: &[("rocket", "Cohete"), ("station", "Estación orbital"), ("whale", "Ballena estelar"), ("none", "Sin protagonista")],
                default: "rocket", random: true, show: Some(Show { id: "world", equals: "space" }), hint: None },
            choice("yaw", "Vista", &[("225", "Frontal"), ("315", "Lateral"), ("45", "Posterior"), ("135", "Cuarta vista")], "225"),
            super::style::look_param(), super::style::richness_param(),
            ParamSpec::Version { current: 1 },
        ]
    }
    fn has_volume(&self)->bool{true}
    fn scene(&self,params:&Params,ctx:&GenContext)->Result<Scene,String>{
        if !ctx.valid(){return Err("Área o rotulador no válido".into());}
        let prm=params.normalized(&self.params());
        Ok(World{space:prm.choice("world")=="space",prm:&prm,ctx,style:Style::new(ctx,1.1).expressive(&prm)}.scene())
    }
    fn generate(&self, params: &Params, ctx: &GenContext) -> Result<GenOutput, String> {
        if !ctx.valid() { return Err("Área o rotulador no válido".into()); }
        let prm = params.normalized(&self.params());
        let world = World { space: prm.choice("world") == "space", prm: &prm, ctx, style: Style::new(ctx, 1.1).expressive(&prm) };
        let scene = world.scene();
        let mut output = world.style.finish(&scene);
        if cfg!(any(test, debug_assertions)) {
            let issues = scene.construction_issues(1.5);
            if !issues.is_empty() { output.warning = Some(issues.join("; ")); }
        }
        Ok(output)
    }
}
