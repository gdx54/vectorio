//! Piezas en metros. Contactos limpios y encastres explícitos; sin estado global.
use crate::{geom::Rng,scene::{Scene,V3}};
use super::{style::{part,Style,Surface},Detail};
mod buildings;
mod landscape;
mod street;
mod crowd;
mod ribbon;
mod vehicle;
mod facade;
mod dome;
mod creature;
mod density;
mod life;
#[cfg(test)] mod tests;
#[cfg(test)] mod dense_tests;
pub use buildings::{house,house_shell,house_flush,house_bare,nave,church,pick,Church,opening,chimney,parapet,eave,bell_tower,stair_head,water_tank,plinth,OpeningKind,Roof,TowerTop};
pub use landscape::{stairs,retaining_wall,terrace,vegetation,terrain,Plant,Cut,Strata};
pub use crowd::{figure,Crowd,Pose,POSES};
pub use ribbon::{ribbon,smooth,resample,zebra,Band,Marking};
pub use vehicle::{vehicle,convoy,Vehicle};
pub use facade::{walls,window_grid,balconies,sign,shopfront,glass_tower,roof_terrace,parasol,Wall,Rhythm,RHYTHMS};
pub use dome::{dome,Dome,Floor};
pub use creature::{tube,creature,rocket,station,Species,SPECIES};
pub use density::{density,Density};
pub use life::{flock,balloon_seller,balloon_bunch,figure_with_shadow};
pub use street::{person,lamp,bench,pot,fountain,pergola,laundry,low_wall,balcony,furrows,paving};
pub fn v(x:f64,y:f64,z:f64)->V3{V3::new(x,y,z)}
pub(super) fn dimensions(min:V3,max:V3)->bool{
    min.finite()&&max.finite()&&max.x>min.x&&max.y>min.y&&max.z>min.z&&
        (max-min).length()<10_000.0
}
pub(super) fn box_piece(scene:&mut Scene,style:Style,min:V3,max:V3,ink:&str){
    if !dimensions(min,max){return;}
    part(scene,ink);let faces=scene.box3(min,max);style.shade(scene,&faces,Surface::Wall,if ink=="terreno"{ink}else{"trama"});
}
/// Pinta un sólido de una pieza ya construido: las paredes y las caras planas con
/// la trama de pared y los faldones inclinados con la de tejado.
pub(super) fn shade_solid(scene:&mut Scene,style:Style,groups:&[Vec<Vec<V3>>],ink:&str){
    let sloped=|group:&&Vec<Vec<V3>>|{let n=super::style::normal(&group[0]);n.z>1e-6&&(style.richness>0.0||n.z<0.999)};
    type Groups=Vec<Vec<Vec<V3>>>;
    let (roofs,walls):(Groups,Groups)=groups.iter().filter(|g|!g.is_empty()).cloned().partition(|g|sloped(&g));
    let hatch=if ink=="terreno"{ink}else{"trama"};
    style.shade_groups(scene,&walls,Surface::Wall,hatch);
    style.shade_groups(scene,&roofs,Surface::Roof,hatch);
}
/// Prisma de perfil cualquiera en la tinta indicada, con su trama.
pub(super) fn prism_piece(scene:&mut Scene,style:Style,profile:&[V3],vector:V3,ink:&str)->Vec<Vec<Vec<V3>>>{
    part(scene,ink);let groups=scene.prism(profile,vector);shade_solid(scene,style,&groups,ink);groups
}
/// Anillo de muros de grosor `t` dentro de la caja [min, max]: un solo sólido, con
/// la corona y la base partidas en cuatro trapecios que no dejan junta.
pub(super) fn ring_faces(min:V3,max:V3,t:f64)->Vec<Vec<V3>>{
    let outer=[v(min.x,min.y,0.0),v(max.x,min.y,0.0),v(max.x,max.y,0.0),v(min.x,max.y,0.0)];
    let inner=[v(min.x+t,min.y+t,0.0),v(max.x-t,min.y+t,0.0),v(max.x-t,max.y-t,0.0),v(min.x+t,max.y-t,0.0)];
    let at=|q:V3,z:f64|v(q.x,q.y,z);
    let mut faces=Vec::new();
    for k in 0..4{
        let j=(k+1)%4;
        faces.push(vec![at(outer[k],min.z),at(outer[j],min.z),at(outer[j],max.z),at(outer[k],max.z)]);
        faces.push(vec![at(inner[k],min.z),at(inner[k],max.z),at(inner[j],max.z),at(inner[j],min.z)]);
        faces.push(vec![at(outer[k],max.z),at(outer[j],max.z),at(inner[j],max.z),at(inner[k],max.z)]);
        faces.push(vec![at(outer[k],min.z),at(inner[k],min.z),at(inner[j],min.z),at(outer[j],min.z)]);
    }
    faces
}
