//! Un frente de agua en tres actos: caserío, plaza abierta y faro. Los almacenes
//! comparten ritmo; los barcos se agrupan en los muelles y el agua deja pausas.
use super::*;
use super::{composition,kit::{self,v,Roof,Plant,Vehicle},style::{self,part,Style,Surface}};
use crate::scene::{Camera,Scene,V3};
pub struct Harbour;

fn block(scene:&mut Scene,style:Style,min:V3,max:V3,roof:Roof,rng:&mut Rng){
    kit::house_shell(scene,style,min,max,roof,rng);
    let rhythm=if rng.f()<0.5{kit::Rhythm::Paired}else{kit::Rhythm::Tall};
    for wall in kit::walls(min,max){
        kit::window_grid(scene,style,wall,2.8,2.8,rhythm);
        if wall.faces_camera(scene)&&wall.width>4.0 {
            if wall.width>5.0 {kit::shopfront(scene,style,wall,0.6,wall.width-1.2,2.8,None,true);}
            else {kit::opening(scene,style,wall.at(wall.width*0.5-0.65,0.0,0.0),wall.along,1.3,2.1,kit::OpeningKind::Dark);}
        }
    }
}
fn build(prm:&Params,ctx:&GenContext,camera:Camera,paper:BBox)->Scene{
    let mut style=Style::new(ctx,0.85).expressive(prm);style.openings=kit::OpeningKind::Dark;
    let mut scene=Scene::new(camera,paper);style.prepare(&mut scene);scene.part("agua");
    scene.set_sun(Some(v(-1.2,-0.3,1.1)));
    // El mar tiene corrientes largas en grupos; no una cuadrícula uniforme.
    let mut water=Rng::new(composition::local_seed(ctx.seed,-1,0));
    part(&mut scene,"agua");
    for j in 0..32 {
        let y=-28.0+j as f64*0.85;
        for i in 0..8 {
            if water.f()<0.35{continue;}
            let x=i as f64*9.0-8.0+water.f()*3.0;
            let run=2.5+water.f()*5.0;
            let line:Vec<_>=(0..=8).map(|k|{let t=k as f64/8.0;v(x+t*run,y+(t*6.0+j as f64*0.5).sin()*0.12,-0.3)}).collect();
            scene.line(&line);
        }
    }
    part(&mut scene,"terreno");
    let land=scene.box3(v(-3.0,0.0,-1.2),v(66.0,44.0,0.0));style.shade(&mut scene,&land,Surface::Wall,"terreno");
    kit::paving(&mut scene,v(0.0,0.4,0.0),v(63.0,6.0,0.0),2.5);
    scene.begin_shadows(0.0,Some([-4.0,-25.0,67.0,45.0]),style.shadow_angle());
    let fortified=prm.choice("port")=="citadel";let industrial=prm.choice("port")=="industrial";
    let count=prm.int("rows");let detail=prm.f64("density");
    for row in 0..count {
        let y=8.0+row as f64*10.0;
        let mut rhythm=Rng::new(composition::local_seed(ctx.seed,row as i32,0));
        for (k,(x0,x1)) in composition::rhythm(0.0,61.0,6.4,&mut rhythm).into_iter().enumerate(){
            // La plaza del faro y una calle transversal permanecen despejadas.
            if (row==0&&x0>38.0)||(fortified&&row<=1&&x1>38.0)||x0<34.0&&x1>28.0{continue;}
            let mut rng=Rng::new(composition::local_seed(ctx.seed,k as i32,row as i32+1));
            if rng.f()>detail{continue;}
            let center=v((x0+x1)*0.5,y+3.0,0.0);
            let back=composition::depth_weight(&camera,center,v(30.0,18.0,0.0),40.0);
            let h=if industrial{5.5}else{3.5+back*5.0+((k+row)%3) as f64*1.3};
            let roof=if industrial||k%4!=0{Roof::Gable}else{Roof::Flat};
            block(&mut scene,style,v(x0,y,0.0),v(x1,y+6.5,h),roof,&mut rng);
            if row==0&&k%2==0{kit::pot(&mut scene,style,v(x0+0.6,y-1.0,0.0),0.65,&mut rng);}
        }
    }
    let mut rng=Rng::new(composition::local_seed(ctx.seed,101,0));
    // El hito queda apartado: no tapa la plaza ni la bocana.
    if fortified{
        for x in [40.0,56.0]{kit::bell_tower(&mut scene,style,v(x,21.0,0.0),4.0,12.0,kit::TowerTop::Spire);}
        part(&mut scene,"contorno");let faces=scene.box3(v(42.0,20.0,0.0),v(54.0,22.0,6.0));style.shade(&mut scene,&faces,Surface::Wall,"trama");
    }else{
        part(&mut scene,"contorno");
        let faces=scene.lathe(v(54.0,10.0,0.0),&[(2.2,0.0),(1.4,12.0)],true);
        style.shade(&mut scene,&faces,Surface::Wall,"sombra");
        scene.lathe(v(54.0,10.0,0.0),&[(2.0,11.5),(2.0,12.0)],true);
        scene.lathe(v(54.0,10.0,0.0),&[(1.3,12.0),(1.3,14.0)],true);
        for k in 0..10 {let a=std::f64::consts::TAU*k as f64/10.0;scene.line(&[v(54.0+a.cos()*1.3,10.0+a.sin()*1.3,12.0),v(54.0+a.cos()*1.3,10.0+a.sin()*1.3,14.0)]);}
        style.lathe(&mut scene,v(54.0,10.0,0.0),&[(1.8,14.0),(0.0,15.5)],"tejados");
    }
    kit::fountain(&mut scene,style,v(42.5,10.0,0.0),1.5);
    for y in [17.5,27.5,37.5]{
        kit::paving(&mut scene,v(0.0,y,0.0),v(64.0,y+1.2,0.0),3.0);
        for x in [28.5,34.0]{kit::pot(&mut scene,style,v(x,y,0.0),0.8,&mut rng);}
    }
    if industrial {
        // Grúa de pórtico, junto al muelle libre, con gancho sobre el agua.
        part(&mut scene,"acento");
        for x in [40.0,48.0]{scene.box3(v(x,1.0,0.0),v(x+0.6,1.6,9.0));}
        scene.box3(v(40.0,-7.0,8.4),v(48.6,1.6,9.0));
        scene.line(&[v(44.0,-6.0,8.4),v(44.0,-6.0,3.0),v(44.5,-6.0,2.7)]);
        for x in (40..48).step_by(2){scene.line(&[v(x as f64,-6.5,8.4),v(x as f64+1.0,1.3,9.0)]);}
    }
    for x in [35.0,62.0]{kit::vegetation(&mut scene,style,v(x,4.0,0.0),4.5,Plant::Palm,&mut rng);}
    for x in [5.0,23.0,45.0] {
        part(&mut scene,"terreno");let faces=scene.box3(v(x,-14.0,-1.0),v(x+2.4,0.0,0.0));
        style.shade(&mut scene,&faces,Surface::Ground,"terreno");
        for y in [-4.0,-10.0]{
            kit::vehicle(&mut scene,Vehicle::Boat,v(x+4.5,y,-0.1),v(0.0,-1.0,0.0));
            part(&mut scene,"acento");let mast=v(x+4.5,y,-0.1)+v(0.0,-2.8,0.0);
            scene.line(&[mast,mast+v(0.0,0.0,5.0)]);
            let sail=[mast+v(0.0,0.0,4.8),mast+v(0.0,0.0,0.8),mast+v(2.5,0.0,0.8)];scene.closed(&sail);
            scene.hatch_face(&sail,v(0.0,0.0,1.0),0.6f64.max(ctx.pen_mm*1.45));
        }
    }
    for x in [8.0,17.0,35.0,47.0]{kit::figure_with_shadow(&mut scene,v(x,3.0,0.0),1.7,kit::Pose::Walk,x>30.0);}
    part(&mut scene,"sombra");scene.hatch_shadows(style.shadow_gap());scene
}

impl Generator for Harbour{
    fn id(&self)->&'static str{"harbour"}
    fn name(&self)->&'static str{"Puerto y ciudadela"}
    fn category(&self)->&'static str{"Escenas"}
    fn description(&self)->&'static str{"Muelles, velas y tejados alrededor de una plaza abierta al agua."}
    fn inks(&self)->&'static InkPlan{&inks::HARBOUR}
    fn params(&self)->Vec<ParamSpec>{vec![
        choice("port","Puerto",&[("fishing","Pesquero"),("industrial","Almacenes"),("citadel","Ciudadela")],"fishing"),
        range("rows","Calles",2.0,4.0,1.0,3.0),range("density","Densidad de casas",0.65,1.0,0.05,0.95),
        choice("yaw","Vista",&[("225","Frontal"),("315","Lateral"),("45","Posterior"),("135","Cuarta vista")],"225"),
        style::look_param(),style::richness_param(),ParamSpec::Version{current:1},
    ]}
    fn has_volume(&self)->bool{true}
    fn scene(&self,params:&Params,ctx:&GenContext)->Result<Scene,String>{
        if !ctx.valid(){return Err("Área o rotulador no válido".into());}
        let prm=params.normalized(&self.params());
        let mut camera=Camera{yaw_deg:prm.choice("yaw").parse().unwrap_or(225.0),..Default::default()};
        camera.fit([v(-10.0,-30.0,-1.5),v(70.0,44.0,18.0)],ctx.area,2.0);
        let scene=fit_scene(ctx.area,camera,|cam,paper|build(&prm,ctx,cam,paper));
        Ok(scene)
    }
    fn generate(&self,params:&Params,ctx:&GenContext)->Result<GenOutput,String>{
        let scene=self.scene(params,ctx)?;
        let prm=params.normalized(&self.params());
        Ok(Style::new(ctx,0.85).expressive(&prm).finish(&scene))
    }
}
