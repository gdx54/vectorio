//! Arquitectura estratificada. Una masa continua se retranquea, se vacía y se
//! equipa por bandas; cada banda tiene un ritmo y deja descansar la siguiente.
use super::*;
use super::{composition, massing::Mass,kit::{self,v},style::{self,part,Style}};
use crate::scene::{Camera,Scene,V3};
pub struct Megastructure;

fn mass(params:&Params,seed:u64)->Mass {
    let height=params.int("height");let mut grid=Mass::new([44,36,height+8]);
    let mut rng=Rng::new(seed);let tiers=params.int("tiers");
    let weights:Vec<f64>=(0..tiers).map(|_|0.65+rng.f()*0.7).collect();
    let total:f64=weights.iter().sum();let mut cumulative=0.0;
    let mut cuts=vec![2];for weight in &weights{cumulative+=weight;cuts.push(2+(height as f64*cumulative/total).round() as usize);}
    let mut x=4;let mut y=4;
    for k in 0..tiers {
        let z=cuts[k];let top=cuts[k+1];let step=top-z;
        let id=if k%2==0{1}else{2};grid.fill([x,y,z],[40,32,top],id);
        // Una crujía recorrible a media altura, con pilares y espalda maciza.
        if step>7 {
            grid.fill([x,y,z+2],[38,y+3,z+5],0);
            for a in (x..38).step_by(7){grid.fill([a,y,z+2],[a+2,y+3,z+5],id);}
        }
        let dense=params.f64("detail");
        for axis in 0..2 {
            let (start,end,wall)=if axis==0{(x,40,y)}else{(y,32,x)};
            let mut bays=Rng::new(composition::local_seed(seed,k as i32,axis));
            let runs=composition::rhythm((start+2) as f64,(end-2) as f64,5.0,&mut bays);
            let mode=(k+axis as usize)%3;
            for (a,b) in runs {
                if bays.f()>dense{continue;}let a=a.ceil() as usize;let b=b.floor() as usize;if b<=a{continue;}
                let at=|u,w,z|if axis==0{[u,w,z]}else{[w,u,z]};
                if mode==0 {
                    // Lamas dentro de un retranqueo, sin rayas sobre el plano frontal.
                    grid.fill(at(a,wall,z+step/2),at(b,wall+2,top-1),0);
                    for h in (z+step/2..top-1).step_by(2){grid.fill(at(a,wall,h),at(b,wall+1,h+1),3);}
                }else if mode==1 {
                    for u in (a..b).step_by(2){grid.fill(at(u,wall.saturating_sub(1),z+step/2),at(u+1,wall,top-1),3);}
                }else {
                    grid.fill(at(a,wall,z+step/2),at(b,wall+2,top-2),0);
                }
            }
        }
        // El retranqueo es desigual: una terraza larga y otra estrecha.
        if k+1<tiers{x+=(1.0+rng.f()*params.f64("setback")) as usize;y+=1+(k%2);}
    }
    let mode=params.choice("structure");
    if mode=="court"{grid.fill([20,12,2],[31,24,height+8],0);}
    if mode=="portal"{grid.fill([14,0,2],[28,36,height/2],0);}
    // La cubierta tiene silueta propia: conductos, baterías de placas y una torre.
    for i in 0..4 {grid.fill([x+2+i*3,y+2,height+2],[x+3+i*3,y+8,height+5],3);}
    grid.fill([34,26,height+2],[38,30,height+7],2);
    grid.fill([1,1,0],[43,35,2],4);grid
}

impl Generator for Megastructure {
    fn id(&self)->&'static str{"megastructure"}
    fn name(&self)->&'static str{"Megastructuras"}
    fn category(&self)->&'static str{"Escenas"}
    fn description(&self)->&'static str{"Estratos, vacíos, pórticos y máquinas en una arquitectura monumental."}
    fn params(&self)->Vec<ParamSpec>{vec![
        choice("structure","Composición",&[("tower","Torre estratificada"),("court","Patio interior"),("portal","Gran pórtico")],"tower"),
        range("height","Altura",28.0,64.0,2.0,44.0),range("tiers","Estratos",2.0,5.0,1.0,4.0),
        range("setback","Retranqueos",1.0,4.0,1.0,3.0),range("detail","Equipamiento",0.2,1.0,0.1,0.85),
        choice("yaw","Vista",&[("225","Frontal"),("315","Lateral"),("45","Posterior"),("135","Cuarta vista")],"225"),
        style::look_param(),style::richness_param(),ParamSpec::Version{current:1},
    ]}
    fn has_volume(&self)->bool{true}
    fn scene(&self,params:&Params,ctx:&GenContext)->Result<Scene,String>{
        if !ctx.valid(){return Err("Área o rotulador no válido".into());}
        let prm=params.normalized(&self.params());let grid=mass(&prm,ctx.seed);
        let style=Style::new(ctx,0.85).expressive(&prm);
        let mut camera=Camera{yaw_deg:prm.choice("yaw").parse().unwrap_or(225.0),..Default::default()};
        camera.fit([v(-12.0,-12.0,0.0),v(53.0,46.0,prm.f64("height")+9.0)],ctx.area,4.0);
        let scene=fit_scene(ctx.area,camera,|camera,paper|{
            let mut scene=Scene::new(camera,paper);style.prepare(&mut scene);
            scene.set_sun(Some(v(-1.2,-0.35,1.0)));
            scene.begin_shadows(0.0,Some([-12.0,-12.0,53.0,46.0]),style.shadow_angle());
            grid.draw(&mut scene,style,V3::default(),1.0);
            // Al pie, objetos pequeños explican la escala sin competir con la torre.
            let mut rng=Rng::new(composition::local_seed(ctx.seed,90,0));
            for x in [6.0,13.0,33.0]{kit::vegetation(&mut scene,style,v(x,-3.5,0.0),3.2,kit::Plant::Cypress,&mut rng);}
            kit::paving(&mut scene,v(2.0,-8.0,0.0),v(40.0,-2.0,0.0),3.0);
            part(&mut scene,"sombra");scene.hatch_shadows(style.shadow_gap());scene
        });
        Ok(scene)
    }
    fn generate(&self,params:&Params,ctx:&GenContext)->Result<GenOutput,String>{
        let scene=self.scene(params,ctx)?;
        let prm=params.normalized(&self.params());
        Ok(Style::new(ctx,0.85).expressive(&prm).finish(&scene))
    }
}
