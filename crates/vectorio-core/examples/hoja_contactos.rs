//! Revisión de 12 semillas por generador:
//! `cargo run -p vectorio-core --example hoja_contactos --release -- contactos.svg`.
mod support;
use std::fmt::Write;
use std::collections::BTreeSet;
use vectorio_core::{p,BBox,geom::Rng,generate::{self,Params,GenContext,Detail}};
fn main()->std::io::Result<()>{
    let path=std::env::args().nth(1).unwrap_or_else(||"hoja_contactos.svg".into());
    let (w,h)=(840.0,generate::all().len() as f64*690.0);
    let mut svg=format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}mm" height="{h}mm" viewBox="0 0 {w} {h}"><rect width="100%" height="100%" fill="white"/>"#);
    for (g,generator) in generate::all().iter().enumerate(){
        let top=g as f64*690.0;
        write!(svg,r#"<text x="12" y="{}" font-family="Helvetica" font-size="7">{}</text>"#,top+12.0,generator.name()).unwrap();
        for seed in 0..12{
            let (x,y)=(10.0+(seed%4) as f64*208.0,top+22.0+(seed/4) as f64*220.0);
            let mut params=Params::default();if seed>0{generator.randomize(&mut Rng::new(seed),&mut params,&BTreeSet::new());}
            let ctx=GenContext{area:BBox{min:p(5.0,5.0),max:p(195.0,195.0)},seed,pen_mm:0.35,detail:Detail::Final};
            let start=std::time::Instant::now();let output=generator.generate(&params,&ctx).map_err(std::io::Error::other)?;
            eprintln!("{} · {seed}: {:?}, {:.2} m{}",generator.name(),start.elapsed(),output.total_m(),output.warning.as_deref().unwrap_or(""));
            write!(svg,r#"<g transform="translate({x},{y})">"#).unwrap();support::colored_paths(&mut svg,&output,generator.inks(),&generator.inks().default_choice());
            write!(svg,r#"<text x="5" y="208" font-family="Helvetica" font-size="4">semilla {seed} · {:.2} m{}</text></g>"#,output.total_m(),if seed==0{" · parámetros de serie"}else{""}).unwrap();
        }
    }
    svg.push_str("</svg>");std::fs::write(path,svg)
}
