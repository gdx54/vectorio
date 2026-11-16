//! Muestrario A4 del kit:
//! `cargo run -p vectorio-core --example kit_piezas --release -- kit_piezas.svg`.
mod support;
use std::fmt::Write;
use vectorio_core::{p,BBox,geom::Rng,scene::{Camera,Scene},generate::{fit_scene,GenContext,Detail,style::Style,kit::{self,v,Roof,Plant,TowerTop}}};
fn main()->std::io::Result<()>{
    let path=std::env::args().nth(1).unwrap_or_else(||"kit_piezas.svg".into());
    let labels=["Casa · dos aguas","Casa · un agua","Casa · cuatro aguas","Azotea y pretil","Campanario · cúpula","Campanario · chapitel",
        "Chimenea y alero","Escalera","Muro y bancal","Ciprés","Olivo y arbusto","Terreno y estratos"];
    let mut svg=String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" width="297mm" height="210mm" viewBox="0 0 297 210"><rect width="297" height="210" fill="white"/>"#);
    for (i,label) in labels.iter().enumerate(){
        let x=5.0+(i%4) as f64*73.0;let y=5.0+(i/4) as f64*68.0;
        let area=BBox{min:p(x+2.0,y+5.0),max:p(x+68.0,y+58.0)};
        let ctx=GenContext{area,seed:i as u64,pen_mm:0.3,detail:Detail::Final};let style=Style::new(&ctx,0.8);
        let mut camera=Camera::default();camera.fit([v(-1.0,-1.0,-1.0),v(6.0,5.0,12.0)],area,1.0);
        let scene=fit_scene(area,camera,|camera,paper|{
            let mut s=Scene::new(camera,paper);style.prepare(&mut s);s.set_sun(Some(v(-1.0,1.0,1.0)));let mut rng=Rng::new(i as u64);
            match i{
                0..=3=>kit::house(&mut s,style,v(0.0,0.0,0.0),v(3.5,3.0,3.0),[Roof::Gable,Roof::Shed,Roof::Hip,Roof::Flat][i],&mut rng),
                4..=5=>kit::bell_tower(&mut s,style,v(2.0,2.0,0.0),2.0,6.0,if i==4{TowerTop::Dome}else{TowerTop::Spire}),
                6=>{kit::house(&mut s,style,v(0.0,0.0,0.0),v(3.5,3.0,3.0),Roof::Gable,&mut rng);
                    kit::chimney(&mut s,style,v(2.0,1.5,3.2),v(2.8,2.3,5.0));kit::eave(&mut s,style,v(-0.2,-0.6,3.0),v(3.7,0.0,3.3));},
                7=>kit::stairs(&mut s,style,v(0.0,0.0,0.0),2.0,4.0,3.0),
                8=>{let (a,b)=(v(0.0,0.0,-1.0),v(5.0,4.0,2.0));let strata=kit::Strata::new(&s,style,a.z,b.z,a,b,&mut rng);
                    kit::terrace(&mut s,style,a,v(5.0,2.0,1.0),kit::Cut{exterior:[true,true,false,true],wall_from:None},&strata);
                    kit::retaining_wall(&mut s,style,v(0.0,2.0,-1.0),v(5.0,4.0,2.0));},
                9=>kit::vegetation(&mut s,style,v(2.0,2.0,0.0),5.0,Plant::Cypress,&mut rng),
                10=>{kit::vegetation(&mut s,style,v(1.0,1.0,0.0),4.0,Plant::Olive,&mut rng);
                    kit::vegetation(&mut s,style,v(4.0,1.0,0.0),1.4,Plant::Shrub,&mut rng);},
                _=>kit::terrain(&mut s,style,v(0.0,0.0,-2.0),v(5.0,4.0,0.0),&mut rng),
            }s
        });
        support::paths(&mut svg,&style.finish(&scene));write!(svg,r#"<text x="{x}" y="{}" font-family="Helvetica" font-size="2.8">{label}</text>"#,y+64.0).unwrap();
    }
    svg.push_str("</svg>");std::fs::write(path,svg)
}
