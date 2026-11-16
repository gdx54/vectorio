use super::*;
use crate::scene::{Camera, Scene, Tone, V3};
use super::{style::{Style,part},kit};
pub struct Blocks;
fn v(x: f64, y: f64, z: f64) -> V3 { V3::new(x, y, z) }
fn normal(face: &[V3]) -> V3 { (face[1] - face[0]).cross(face[2] - face[0]).normalized() }
fn shade(s: &mut Scene, faces: Vec<Vec<V3>>, spacing: f64, curved: bool) {
    let mut groups = [Vec::new(), Vec::new(), Vec::new()];
    for f in faces { let i = match s.tone(normal(&f)) { Tone::Lit => 0, Tone::Mid => 1, Tone::Dark => 2 }; groups[i].push(f); }
    for (i, faces) in groups.iter().enumerate() {
        let part = s.part(["trama clara", "trama media", "trama oscura"][i]); s.set_part(part);
        let gap = spacing * [2.5, 1.5, 1.0][i];
        if curved { s.hatch_surface(faces, 0.0, gap); }
        else { for f in faces { s.hatch_face(f, f[1] - f[0], gap); } }
    }
    s.set_part(crate::scene::PartId(0));
}
impl Generator for Blocks {
    fn id(&self) -> &'static str { "blocks" }
    fn name(&self) -> &'static str { "Manzanas" }
    fn category(&self) -> &'static str { "Escenas" }
    fn description(&self) -> &'static str { "Bloques, plazas y torres bajo el sol." }
    fn params(&self) -> Vec<ParamSpec> { vec![
        ParamSpec::Section { label: "Distribución" },
        range("rows", "Filas", 1.0, 8.0, 1.0, 4.0), range("cols", "Columnas", 1.0, 8.0, 1.0, 4.0),
        range("min_height", "Altura mínima", 0.5, 4.0, 0.1, 0.8), range("max_height", "Altura máxima", 1.0, 6.0, 0.1, 3.5),
        range("gap", "Probabilidad de plaza", 0.0, 0.7, 0.05, 0.15), range("street", "Ancho de calle", 0.2, 1.5, 0.1, 0.6),
        ParamSpec::Toggle { id: "towers", label: "Torres", default: true, random: Some(0.8), show: None, hint: None },
        ParamSpec::Section { label: "Cámara y luz" },
        choice("yaw", "Vista", &[("45", "Frontal"), ("135", "Lateral"), ("225", "Posterior")], "45"),
        choice("elevation", "Elevación", &[("35.26438968", "Isométrica"), ("26.57", "Dimétrica")], "35.26438968"),
        range("sun", "Ángulo del sol", 0.0, 360.0, 5.0, 140.0), range("sun_height", "Altura del sol", 20.0, 70.0, 5.0, 45.0), range("spacing", "Separación de trama", 0.6, 2.5, 0.1, 1.0),
        super::style::look_param(),super::style::richness_param(),
        ParamSpec::Version { current: 3 },
    ] }
    fn has_volume(&self)->bool{true}
    fn scene(&self, params: &Params, ctx: &GenContext) -> Result<Scene, String> {
        if !ctx.valid() { return Err("Área o rotulador no válido".into()); }
        let prm = params.normalized(&self.params());
        let (rows, cols) = (prm.int("rows"), prm.int("cols"));
        let street = prm.f64("street"); let pitch = 2.0 + street;
        let (w, h) = (cols as f64 * pitch, rows as f64 * pitch);
        let (min, max) = (prm.f64("min_height").min(prm.f64("max_height")), prm.f64("min_height").max(prm.f64("max_height")));
        let mut cam = Camera { yaw_deg: prm.choice("yaw").parse().unwrap_or(45.0),
            elev_deg: prm.choice("elevation").parse().unwrap_or(35.264), ..Camera::default() };
        cam.fit([v(-3.0, -3.0, -0.25), v(w + 3.0, h + 3.0, max + 1.6)], ctx.area, 1.0);
        let legacy=prm.f64(SCENE_VERSION)<3.0;
        let style=Style::new(ctx,prm.f64("spacing")).expressive(&prm);
        let build=|cam,paper| {
        let mut s = Scene::new(cam, paper);
        if legacy {s.set_intersections(false);} else {style.prepare(&mut s);}
        if legacy {for name in ["tejados", "trama clara", "trama media", "trama oscura", "sombra"] { s.part(name); }}
        let angle = prm.f64("sun").to_radians();
        let sun_height=prm.f64("sun_height").to_radians();
        s.set_sun(Some(if legacy {v(angle.cos(),angle.sin(),1.5)}else{v(angle.cos()*sun_height.cos(),angle.sin()*sun_height.cos(),sun_height.sin())}));
        if legacy {s.box3(v(0.0, 0.0, -0.25), v(w, h, 0.0));}
        else {kit::terrain(&mut s,style,v(0.0,0.0,-1.0),v(w,h,0.0),&mut Rng::new(ctx.seed.wrapping_add(1)));}
        s.begin_shadows(0.0, Some([0.0, 0.0, w, h]), 20.0);
        let mut rng = Rng::new(ctx.seed);
        let spacing = prm.f64("spacing").max(ctx.pen_mm * 1.8) * if ctx.detail == Detail::Preview { 1.15 } else { 1.0 };
        for y in 0..rows { for x in 0..cols {
            if rng.f() < prm.f64("gap") { continue; }
            let (x, y) = (x as f64 * pitch + street * 0.5, y as f64 * pitch + street * 0.5);
            let weight=super::composition::depth_weight(&cam,v(x,y,0.0),v(w*0.5,h*0.5,0.0),w.max(h));
            let roll=rng.f();let height = min + (if legacy{roll}else{roll*0.65+weight*0.35}) * (max - min);
            if prm.flag("towers") && rng.f() < 0.18 && s.visible(0.6, 1.0) {
                if legacy {
                    let faces = s.lathe(v(x + 1.0, y + 1.0, 0.0), &[(0.75, 0.0), (0.75, height)], true);
                    shade(&mut s, faces, spacing, true);
                    let roof = s.part("tejados"); s.set_part(roof);
                    let faces = s.lathe(v(x + 1.0, y + 1.0, 0.0), &[(0.85, height), (0.0, height + 1.2)], true);
                    shade(&mut s, faces, spacing, true);
                } else {
                    style.lathe(&mut s,v(x+1.0,y+1.0,0.0),&[(0.75,0.0),(0.75,height)],"contorno");
                    // Pendiente de 60,5°: distinta de los complementos de ambas cámaras.
                    style.lathe(&mut s,v(x+1.0,y+1.0,0.0),&[(0.85,height),(0.0,height+1.5)],"detalle");
                    part(&mut s,"contorno");
                }
            } else {
                if legacy {let faces=s.box3(v(x,y,0.0),v(x+2.0,y+2.0,height));shade(&mut s,faces,spacing,false);}
                else {let roof=if rng.f()<0.65{kit::Roof::Gable}else{kit::Roof::Flat};
                    kit::house(&mut s,style,v(x,y,0.0),v(x+2.0,y+2.0,height),roof,&mut rng);part(&mut s,"contorno");}
            }
        } }
        let shadows = s.part("sombra"); s.set_part(shadows); s.hatch_shadows(if legacy{spacing*1.5}else{style.shadow_gap()});
        s
        };
        let scene=if legacy {build(cam,ctx.area)}else{fit_scene(ctx.area,cam,build)};
        Ok(scene)
    }
    fn generate(&self,params:&Params,ctx:&GenContext)->Result<GenOutput,String>{
        let scene=self.scene(params,ctx)?;
        let prm=params.normalized(&self.params());
        let legacy=prm.f64(SCENE_VERSION)<3.0;
        let style=Style::new(ctx,prm.f64("spacing")).expressive(&prm);
        if !legacy {return Ok(style.finish(&scene));}
        let mut output:GenOutput=scene.render().into();
        if let Some(part)=output.parts.last_mut(){part.pen_order=Some(usize::MAX);}
        Ok(output)
    }
}
