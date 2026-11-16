//! Juegos de tintas del Pueblo blanco: filas por juego, columnas por número de rotuladores.
//! `cargo run -p vectorio-core --example juegos_tintas --release -- tintas.svg [semilla]`.
use std::fmt::Write;
use vectorio_core::{p,BBox,generate::{self,Params,GenContext,Detail,InkChoice}};
fn main()->std::io::Result<()>{
    let mut args=std::env::args().skip(1);
    let path=args.next().unwrap_or_else(||"juegos_tintas.svg".into());
    let seed=args.next().and_then(|s|s.parse().ok()).unwrap_or(3);
    let generator=generate::find("white_village").unwrap();let plan=generator.inks();
    let ctx=GenContext{area:BBox{min:p(4.0,8.0),max:p(196.0,156.0)},seed,pen_mm:0.3,detail:Detail::Final};
    let output=generator.generate(&Params::default(),&ctx).map_err(std::io::Error::other)?;
    let (cols,rows)=(4,plan.palettes.len());let (w,h)=(cols as f64*200.0,rows as f64*160.0);
    let mut svg=format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}mm" height="{h}mm" viewBox="0 0 {w} {h}"><rect width="100%" height="100%" fill="white"/>"#);
    for (r,palette) in plan.palettes.iter().enumerate(){for count in 1..=cols{
        let choice=InkChoice{palette:palette.id.into(),count};
        let pens=plan.assign(&choice,output.parts.iter().map(|p|p.name.as_str()));
        write!(svg,r#"<g transform="translate({},{})">"#,(count-1) as f64*200.0,r as f64*160.0).unwrap();
        for part in &output.parts{
            let pen=&pens[&part.name];let [cr,cg,cb]=pen.color;
            write!(svg,r##"<g fill="none" stroke="#{cr:02x}{cg:02x}{cb:02x}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round">"##,pen.width_mm*0.8).unwrap();
            for line in &part.lines{svg.push_str("<polyline points=\"");for q in line{write!(svg,"{:.3},{:.3} ",q.x,q.y).unwrap();}svg.push_str("\"/>");}
            svg.push_str("</g>");
        }
        write!(svg,r#"<text x="6" y="7" font-family="Helvetica" font-size="5">{} · {count}</text></g>"#,palette.name).unwrap();
    }}
    svg.push_str("</svg>");std::fs::write(path,svg)
}
