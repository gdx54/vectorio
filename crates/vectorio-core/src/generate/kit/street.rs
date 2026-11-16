//! Piezas menudas que articulan calles y plazas: gente, farolas, bancos, macetas,
//! fuente, pérgolas, tendederos, tapias, balcones y surcos. Todas en metros y con
//! contactos limpios; las que quedarían por debajo de un milímetro no se dibujan.
use super::*;

fn ok(origin:V3,sizes:&[f64])->bool{origin.finite()&&sizes.iter().all(|n|n.is_finite()&&*n>0.0&&*n<1000.0)}
/// Separación mínima en papel entre caras paralelas que se ven a la vez: por debajo
/// de 1,5 mm dos caras se leen como una sola con un trazo doble.
pub(super) fn apart(scene:&Scene)->f64{1.6/scene.camera().scale}
fn plain_box(scene:&mut Scene,min:V3,max:V3,ink:&str){if dimensions(min,max){part(scene,ink);scene.box3(min,max);}}

/// Figura de pie: cuerpo, cuello y cabeza en un solo torno; arroja su sombra.
pub fn person(scene:&mut Scene,origin:V3,height:f64){
    if !ok(origin,&[height])||!scene.visible(height,2.2){return;}
    let h=height/1.72;
    let profile=[(0.12*h,0.0),(0.16*h,0.5*h),(0.2*h,1.0*h),(0.21*h,1.3*h),(0.07*h,1.42*h),(0.11*h,1.5*h),(0.1*h,1.63*h),(0.0,1.72*h)];
    part(scene,"detalle");scene.lathe(origin,&profile,true);
}
/// Farola de poste fino con farol; el poste proyecta una sombra larga.
pub fn lamp(scene:&mut Scene,origin:V3,height:f64){
    if !ok(origin,&[height])||!scene.visible(height,4.0){return;}
    let t=0.06f64.max(0.25/scene.camera().scale);let lantern=0.28f64.max(t*3.0);
    plain_box(scene,origin-v(t,t,0.0),origin+v(t,t,height),"contorno");
    let top=origin+v(0.0,0.0,height);
    plain_box(scene,top-v(lantern*0.5,lantern*0.5,0.0),top+v(lantern*0.5,lantern*0.5,(lantern*1.3).max(apart(scene))),"contorno");
}
/// Banco de obra con respaldo, de una pieza.
/// `along_x` orienta el asiento; el respaldo queda en el lado de y (o x) mayor.
pub fn bench(scene:&mut Scene,origin:V3,along_x:bool){
    if !ok(origin,&[1.0])||!scene.visible(1.5,4.0){return;}
    // El respaldo queda a 1,6 mm del frente del asiento como mínimo.
    let back=0.4f64.max(apart(scene)*1.05);let d=back+0.15f64.max(apart(scene)*1.05);
    let l=1.5;let size=|a:f64,b:f64|if along_x{v(a,b,0.0)}else{v(b,a,0.0)};
    let seat=0.45f64.max(apart(scene));let top=seat+0.45f64.max(apart(scene));
    // Asiento y respaldo son un prisma de perfil en L: sin junta entre los dos.
    let at=|depth:f64,z:f64|origin+size(0.0,depth)+v(0.0,0.0,z);
    let profile=[at(0.0,0.0),at(d,0.0),at(d,top),at(back,top),at(back,seat),at(0.0,seat)];
    part(scene,"contorno");scene.prism(&profile,size(l,0.0));
}
/// Maceta con una mata; la mata se hunde en la tierra de la maceta.
pub fn pot(scene:&mut Scene,style:Style,origin:V3,size:f64,rng:&mut Rng){
    if !ok(origin,&[size])||!scene.visible(size,2.0){return;}
    // La boca queda al menos a 1,6 mm del suelo: no es una rebanada del pavimento.
    let pot=(size*0.5).max(1.6/scene.camera().scale);
    part(scene,"contorno");scene.lathe(origin,&[(size*0.32,0.0),(size*0.42,pot)],false);
    let q=style.quantum(scene);let plant=(size*1.1).max(q*2.0);
    let radius=plant*(0.45+rng.f()*0.15);
    style.lathe(scene,origin+v(0.0,0.0,pot-q*0.2),&[(size*0.36,0.0),(radius,plant*0.4),(radius*0.6,plant*0.85),(0.0,plant)],"vegetación");
}
/// Fuente de plaza: pilón, borde interior, fuste y taza.
pub fn fountain(scene:&mut Scene,style:Style,origin:V3,radius:f64){
    if !ok(origin,&[radius])||!scene.visible(radius,3.0){return;}
    let q=style.quantum(scene);let rim=0.5f64.max(q);
    part(scene,"contorno");scene.lathe(origin,&[(radius,0.0),(radius,rim)],false);
    let n=scene.circle_segments(radius);let inner=radius-0.18f64.max(0.6/scene.camera().scale);
    let ring:Vec<_>=(0..=n).map(|i|{let a=i as f64/n as f64*std::f64::consts::TAU;origin+v(inner*a.cos(),inner*a.sin(),rim)}).collect();
    part(scene,"detalle");scene.closed(&ring);
    // Ondas: dos anillos discontinuos sobre el agua.
    for (k,r) in [(0,inner*0.72),(1,inner*0.48)]{
        for arc in 0..6{
            let a0=(arc as f64+k as f64*0.5)/6.0*std::f64::consts::TAU;
            let points:Vec<_>=(0..=6).map(|i|{let a=a0+i as f64/6.0*0.6;origin+v(r*a.cos(),r*a.sin(),rim)}).collect();
            scene.line(&points);
        }
    }
    // Fuste y taza en un solo torno: no queda una tapa oculta bajo la taza.
    let shaft=radius*0.14;let top=rim+radius*0.7;let lip=(radius*0.06).max(apart(scene)*0.5);
    part(scene,"contorno");scene.lathe(origin,&[(shaft,rim-q*0.3),(shaft,top),(radius*0.42,top+radius*0.16),(radius*0.42,top+radius*0.16+lip)],true);
}
/// Pérgola de dos pórticos (pies y viga) con listones como líneas.
pub fn pergola(scene:&mut Scene,min:V3,max:V3,height:f64){
    if !dimensions(min,max+v(0.0,0.0,1.0))||!ok(min,&[height])||!scene.visible(height,4.0){return;}
    let t=0.07f64.max(0.3/scene.camera().scale);let z=min.z;
    // Los pies opuestos y las dos vigas guardan 1,6 mm en papel entre sí.
    if (max.x-min.x).min(max.y-min.y)-t*2.0<apart(scene)*1.05{return;}
    // Cada pórtico (dos pies y su viga) es un prisma de una pieza: la viga no deja
    // junta sobre los pies.
    let beam=(t*2.5).max(apart(scene));let (low,high)=(z+height,z+height+beam);
    let frame=[(min.x,z),(min.x+t*2.0,z),(min.x+t*2.0,low),(max.x-t*2.0,low),(max.x-t*2.0,z),(max.x,z),
        (max.x,low),(max.x+t,low),(max.x+t,high),(min.x-t,high),(min.x-t,low),(min.x,low)];
    part(scene,"contorno");
    for y in [min.y,max.y-t*2.0]{
        let profile:Vec<V3>=frame.iter().map(|&(x,z)|v(x,y,z)).collect();
        scene.prism(&profile,v(0.0,t*2.0,0.0));
    }
    part(scene,"detalle");let count=((max.x-min.x)/0.35).round().clamp(2.0,40.0) as usize;
    for i in 1..count{let x=min.x+(max.x-min.x)*i as f64/count as f64;
        scene.line(&[v(x,min.y-t,z+height+beam),v(x,max.y+t,z+height+beam)]);}
}
/// Tendedero entre dos postes; las prendas son trazos atómicos de color de acento.
pub fn laundry(scene:&mut Scene,a:V3,b:V3,height:f64,rng:&mut Rng){
    if !a.finite()||!b.finite()||!ok(a,&[height])||!scene.visible(height,4.0)||(b-a).length()<0.8{return;}
    let t=0.04f64.max(0.25/scene.camera().scale);
    for p in [a,b]{plain_box(scene,p-v(t,t,0.0),p+v(t,t,height),"contorno");}
    let (ta,tb)=(a+v(0.0,0.0,height*0.95),b+v(0.0,0.0,height*0.95));
    part(scene,"detalle");scene.line(&[ta,(ta+tb)*0.5-v(0.0,0.0,0.06),tb]);
    let along=(b-a).normalized();let span=(b-a).length();
    let mut s=0.25;part(scene,"acento");
    while s<span-0.5{
        let w=0.3+rng.f()*0.25;let h=0.35+rng.f()*0.35;let top=ta+along*s-v(0.0,0.0,0.05);
        scene.whole_line(&[top,top+along*w,top+along*w-v(0.0,0.0,h),top-v(0.0,0.0,h),top]);
        s+=w+0.12+rng.f()*0.2;
    }
}
/// Tapia de patio o de huerto: tramos sin solapes, con hueco de entrada al frente.
/// `sides` = [izquierda, fondo, derecha]; un lado apagado es la pared de otra pieza
/// y no se duplica con un muro pegado a ella.
pub fn low_wall(scene:&mut Scene,style:Style,min:V3,max:V3,height:f64,gate:Option<(f64,f64)>,sides:[bool;3]){
    if !dimensions(min,max+v(0.0,0.0,1.0))||!ok(min,&[height]){return;}
    // Los lados van de punta a punta y el frente y el fondo encajan entre ellos:
    // ninguna testa queda escondida a menos de 1,5 mm detrás de otra cara.
    let t=0.22f64.max(apart(scene));if (max.x-min.x).min(max.y-min.y)<t*4.0{return;}
    let z=min.z;let top=z+height.max(apart(scene));
    let x0=if sides[0]{min.x+t}else{min.x};let x1=if sides[2]{max.x-t}else{max.x};
    let gate=gate.filter(|&(a,b)|a>x0+t&&b<x1-t&&b>a);
    let present=[true,sides[2],sides[1],sides[0]];
    // Sin puerta y con los cuatro lados, un anillo; si no, cada tramo seguido de
    // tapia es un prisma: las esquinas no dejan junta.
    if gate.is_none()&&present.iter().all(|p|*p){
        part(scene,"contorno");
        let groups=scene.polyhedron(&ring_faces(v(min.x,min.y,z),v(max.x,max.y,top),t));
        shade_solid(scene,style,&groups,"contorno");
        return;
    }
    for outline in wall_outlines(min,max,t,gate,present){
        prism_piece(scene,style,&outline,v(0.0,0.0,top-z),"contorno");
    }
}
/// Tramo recto de tapia en planta: inicio, fin y normal hacia dentro.
struct Run{start:V3,end:V3,inward:V3}
/// Plantas de los tramos seguidos de una tapia alrededor de [min, max] a la cota
/// `min.z`. `present` sigue el orden frente, derecha, fondo, izquierda; la puerta
/// parte el frente.
fn wall_outlines(min:V3,max:V3,t:f64,gate:Option<(f64,f64)>,present:[bool;4])->Vec<Vec<V3>>{
    let z=min.z;
    let corners=[v(min.x,min.y,z),v(max.x,min.y,z),v(max.x,max.y,z),v(min.x,max.y,z)];
    let inward=[v(0.0,1.0,0.0),v(-1.0,0.0,0.0),v(0.0,-1.0,0.0),v(1.0,0.0,0.0)];
    let side=|k:usize|Some(Run{start:corners[k],end:corners[(k+1)%4],inward:inward[k]});
    // Recorrido cerrado que empieza justo después de un hueco; `None` es un hueco.
    let mut runs:Vec<Option<Run>>=match gate{
        Some((a,b))=>vec![
            Some(Run{start:v(b,min.y,z),end:corners[1],inward:inward[0]}),
            side(1).filter(|_|present[1]),side(2).filter(|_|present[2]),side(3).filter(|_|present[3]),
            Some(Run{start:corners[0],end:v(a,min.y,z),inward:inward[0]}),
            None,
        ],
        None=>{
            let first=(0..4).find(|&k|!present[k]).map_or(0,|k|(k+1)%4);
            (0..4).map(|i|(first+i)%4).map(|k|side(k).filter(|_|present[k])).collect()
        }
    };
    runs.push(None);
    let mut outlines=Vec::new();
    let mut chain:Vec<Run>=Vec::new();
    for run in runs{
        match run{
            Some(run)=>chain.push(run),
            None if !chain.is_empty()=>outlines.push(chain_outline(&std::mem::take(&mut chain),t)),
            None=>{}
        }
    }
    outlines
}
/// Contorno de una cadena de tramos: la cara de fuera de ida y la de dentro de
/// vuelta, con las esquinas interiores desplazadas en los dos ejes.
fn chain_outline(chain:&[Run],t:f64)->Vec<V3>{
    let mut outer=vec![chain[0].start];
    let mut inner=vec![chain[0].start+chain[0].inward*t];
    for (i,run) in chain.iter().enumerate(){
        outer.push(run.end);
        let next=chain.get(i+1).map_or(V3::default(),|next|next.inward);
        inner.push(run.end+(run.inward+next)*t);
    }
    outer.extend(inner.into_iter().rev());
    outer
}
/// Balcón volado: losa en contacto con la fachada y barandilla de barrotes.
pub fn balcony(scene:&mut Scene,style:Style,origin:V3,width:f64,depth:f64){
    if !ok(origin,&[width,depth])||!scene.visible(width,4.0){return;}
    let slab=0.12f64.max(style.quantum(scene)*0.25);let depth=depth.max(apart(scene));
    plain_box(scene,origin-v(0.0,depth,0.0),origin+v(width,0.0,slab),"contorno");
    let z=origin.z+slab;let rail=0.95;let y=origin.y-depth;
    part(scene,"detalle");
    scene.line(&[v(origin.x,origin.y,z+rail),v(origin.x,y,z+rail),v(origin.x+width,y,z+rail),v(origin.x+width,origin.y,z+rail)]);
    let count=(width/0.18).round().clamp(3.0,60.0) as usize;
    for i in 0..=count{let x=origin.x+width*i as f64/count as f64;scene.line(&[v(x,y,z),v(x,y,z+rail)]);}
}
/// Surcos paralelos sobre un suelo horizontal, en la tinta de la vegetación.
pub fn furrows(scene:&mut Scene,min:V3,max:V3,along_x:bool,spacing:f64){
    if !min.finite()||!max.finite()||max.x<=min.x||max.y<=min.y||!spacing.is_finite()||spacing<=0.0{return;}
    let spacing=spacing.max(0.9/scene.camera().scale);
    part(scene,"vegetación");
    let (a,b)=if along_x{(min.y,max.y)}else{(min.x,max.x)};
    let count=((b-a)/spacing).floor().min(400.0) as usize;
    for i in 1..=count{let c=a+i as f64*spacing;if b-c<spacing*0.4{break;}
        if along_x{scene.line(&[v(min.x,c,min.z),v(max.x,c,min.z)]);}else{scene.line(&[v(c,min.y,min.z),v(c,max.y,min.z)]);}}
}
/// Enlosado de plaza: retícula de juntas sobre el suelo.
pub fn paving(scene:&mut Scene,min:V3,max:V3,step:f64){
    if !min.finite()||!max.finite()||max.x<=min.x||max.y<=min.y||!step.is_finite()||step<=0.0{return;}
    part(scene,"terreno");let step=step.max(1.5/scene.camera().scale);
    let mut x=min.x+step;while x<max.x-step*0.3{scene.line(&[v(x,min.y,min.z),v(x,max.y,min.z)]);x+=step;}
    let mut y=min.y+step;while y<max.y-step*0.3{scene.line(&[v(min.x,y,min.z),v(max.x,y,min.z)]);y+=step;}
}
