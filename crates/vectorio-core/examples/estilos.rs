//! Pueblo blanco en Grabado y en Cartel, con sus tintas: filas por semilla.
//! `cargo run -p vectorio-core --example estilos --release -- estilos.svg [semillas…]`.
//! `VECTORIO_FRAME=model` dibuja la maqueta de terreno cortado en lugar del encuadre a sangre.
//! `VECTORIO_CELL=210x297` cambia el tamaño de cada celda en mm (300x230 por defecto).
//! `VECTORIO_SET="relief=flat,shadow=2"` fija otros parámetros (número o texto).
use std::fmt::Write;
use vectorio_core::{p,BBox,generate::{self,Params,ParamValue,GenContext,Detail,InkChoice}};
fn main()->std::io::Result<()>{
    let mut args=std::env::args().skip(1);
    let path=args.next().unwrap_or_else(||"estilos.svg".into());
    let mut seeds:Vec<u64>=args.filter_map(|s|s.parse().ok()).collect();if seeds.is_empty(){seeds=vec![3,7,11];}
    let generator=generate::find("white_village").unwrap();let plan=generator.inks();
    let columns=[("engraving","lime_indigo"),("poster","poster"),("poster","lime_tile")];
    let (cw,ch)=std::env::var("VECTORIO_CELL").ok().and_then(|c|{let (a,b)=c.split_once('x')?;Some((a.parse().ok()?,b.parse().ok()?))}).unwrap_or((300.0,230.0));let (w,h)=(columns.len() as f64*cw,seeds.len() as f64*ch);
    let mut svg=format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}mm" height="{h}mm" viewBox="0 0 {w} {h}"><rect width="100%" height="100%" fill="white"/>"#);
    for (r,seed) in seeds.iter().enumerate(){for (c,(look,palette)) in columns.iter().enumerate(){
        let mut params=Params::default();params.set("look",ParamValue::Text((*look).into()));
        if let Ok(frame)=std::env::var("VECTORIO_FRAME"){params.set("frame",ParamValue::Text(frame));}
        for (k,value) in std::env::var("VECTORIO_SET").unwrap_or_default().split(',').filter_map(|kv|kv.split_once('=')){
            params.set(k.trim(),value.trim().parse::<f64>().map(ParamValue::Number).unwrap_or_else(|_|ParamValue::Text(value.trim().into())));
        }
        let ctx=GenContext{area:BBox{min:p(6.0,12.0),max:p(cw-6.0,ch-6.0)},seed:*seed,pen_mm:0.3,detail:Detail::Final};
        let output=generator.generate(&params,&ctx).map_err(std::io::Error::other)?;
        let choice=InkChoice{palette:(*palette).into(),count:5};
        let pens=plan.assign(&choice,output.parts.iter().map(|p|p.name.as_str()));
        write!(svg,r#"<g transform="translate({},{})">"#,c as f64*cw,r as f64*ch).unwrap();
        for part in &output.parts{
            let pen=&pens[&part.name];let [cr,cg,cb]=pen.color;
            write!(svg,r##"<g fill="none" stroke="#{cr:02x}{cg:02x}{cb:02x}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round">"##,pen.width_mm*0.8).unwrap();
            for line in &part.lines{svg.push_str("<polyline points=\"");for q in line{write!(svg,"{:.3},{:.3} ",q.x,q.y).unwrap();}svg.push_str("\"/>");}
            svg.push_str("</g>");
        }
        write!(svg,r#"<text x="8" y="9" font-family="Helvetica" font-size="5">{look} · {palette} · semilla {seed} · {:.1} m</text></g>"#,output.total_m()).unwrap();
    }}
    svg.push_str("</svg>");std::fs::write(path,svg)
}
