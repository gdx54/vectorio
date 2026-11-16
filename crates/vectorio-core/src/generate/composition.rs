//! Reglas comunes: jerarquía, ritmos agrupados y azar local independiente.
use crate::{geom::Rng, scene::{Camera,V3}};

/// Cada parcela conserva sus decisiones aunque otra añada o quite detalles.
pub fn local_seed(seed:u64,x:i32,y:i32)->u64 {
    let mut h=seed^(x as u64).wrapping_mul(0x9e3779b97f4a7c15)^(y as u64).wrapping_mul(0xbf58476d1ce4e5b9);
    h=(h^(h>>30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h=(h^(h>>27)).wrapping_mul(0x94d049bb133111eb);h^(h>>31)
}
/// Tramos desiguales pero relacionados: grupos de dos o tres y una pausa.
pub fn rhythm(start:f64,end:f64,pitch:f64,rng:&mut Rng)->Vec<(f64,f64)> {
    if !pitch.is_finite()||pitch<=0.0||end<=start{return Vec::new();}
    let mut runs=Vec::new();let mut x=start;let mut group=0;
    while end-x>pitch*0.55&&runs.len()<128 {
        let w=(pitch*[0.8,1.0,1.25][(rng.f()*3.0) as usize]).min(end-x);
        runs.push((x,x+w));x+=w;group+=1;
        if group==3{x+=pitch*0.5;group=0;}else{x+=pitch*0.08;}
    }
    runs
}
/// Un hito gana altura hacia el fondo: protege las calles y el foco delantero.
pub fn depth_weight(camera:&Camera,point:V3,center:V3,radius:f64)->f64 {
    let d=(point-center).dot(camera.axes().2)/radius.max(1e-6);
    (0.5+d*0.65).clamp(0.0,1.0)
}
