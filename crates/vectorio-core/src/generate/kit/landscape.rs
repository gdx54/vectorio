use super::*;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
/// Especies: ciprés, olivo, mata, palmera (copa de hojas sueltas) y pino
/// piñonero (copa plana y ancha sobre un tronco alto).
pub enum Plant{Cypress,Olive,Shrub,Palm,Pine}
/// Bloque de maqueta suelto, con estratos en sus cuatro caras.
pub fn terrain(scene:&mut Scene,style:Style,min:V3,max:V3,rng:&mut Rng){
    if !dimensions(min,max){return;}
    let strata=Strata::new(scene,style,min.z,max.z,min,max,rng);
    terrace(scene,style,min,max,Cut::block(),&strata);
}
pub fn retaining_wall(scene:&mut Scene,style:Style,min:V3,max:V3){box_piece(scene,style,min,max,"terreno");}
/// Caras de un bancal, en el orden delantera (`min.y`), derecha, trasera e
/// izquierda: las que dan al exterior de la maqueta llevan estratos. Si la delantera
/// es un muro de contención, `wall_from` es la cota desde la que se ve.
#[derive(Clone,Copy,Debug)]
pub struct Cut{pub exterior:[bool;4],pub wall_from:Option<f64>}
impl Cut{
    /// Bloque suelto: las cuatro caras al exterior.
    pub fn block()->Self{Self{exterior:[true;4],wall_from:None}}
}
/// Una capa del terreno: cota media, fase propia y, si es una lente, el tramo
/// (centro y semilongitud en `x + y`) donde existe.
#[derive(Clone,Copy,Debug)]
struct Layer{base:f64,phase:f64,lens:Option<(f64,f64)>}
/// Estratos de un terreno cortado, comunes a todos los bancales: cada capa sigue
/// de un bloque al siguiente y dobla las esquinas sin saltos, porque su altura es
/// un campo del mundo y no de la cara.
#[derive(Clone,Debug)]
pub struct Strata{step:f64,phases:[f64;2],layers:Vec<Layer>}
impl Strata{
    /// Capas entre `bottom` y `top` en un terreno que ocupa `[min, max]` en planta.
    pub fn new(scene:&Scene,style:Style,bottom:f64,top:f64,min:V3,max:V3,rng:&mut Rng)->Self{
        let preview=if style.detail==Detail::Preview{1.5}else{1.0};
        let step=(1.2/scene.camera().scale).max(0.22)*preview;
        let tau=std::f64::consts::TAU;
        let phases=[rng.f()*tau,rng.f()*tau];
        let (u0,u1)=(min.x+min.y,max.x+max.y);
        let mut layers=Vec::new();
        let mut z=bottom+step*(0.4+rng.f()*0.6);
        while z<top&&layers.len()<2000{
            // Grosor irregular y, de vez en cuando, una lente que se acuña y desaparece.
            let lens=(rng.f()<0.15).then(||{
                let center=u0+(u1-u0)*rng.f();
                (center,(u1-u0).abs()*(0.08+rng.f()*0.12)+1.0)
            });
            layers.push(Layer{base:z,phase:rng.f()*tau,lens});
            z+=step*(0.7+rng.f()*0.9);
        }
        Self{step,phases,layers}
    }
    /// Altura de la capa sin lente: una ondulación común de baja frecuencia más otra
    /// propia y menor; juntas no pasan del 30 % del paso, así que no se cruzan.
    fn plain(&self,k:usize,x:f64,y:f64)->f64{
        let layer=self.layers[k];
        let shared=(x*0.31+y*0.17+self.phases[0]).sin()+0.45*(x*0.11-y*0.23+self.phases[1]).sin();
        let own=(x*0.53+y*0.41+layer.phase).sin();
        layer.base+self.step*(0.15*shared+0.08*own)
    }
    /// Altura de la capa y si se dibuja en ese punto. Una lente se adelgaza hasta
    /// juntarse con la capa de encima.
    fn height(&self,k:usize,x:f64,y:f64)->(f64,bool){
        let plain=self.plain(k,x,y);
        let Some((center,half))=self.layers[k].lens else{return (plain,true)};
        if k+1>=self.layers.len(){return (plain,true);}
        let t=(x+y-center)/half;
        let open=(1.0-t*t).max(0.0);
        let above=self.plain(k+1,x,y);
        (above-(above-plain)*open,open>0.05)
    }
    /// Estratos sobre la cara vertical que va de `a` a `b`, entre las cotas `low` y
    /// `high`; cada capa se parte donde sale de la cara o se cierra su lente.
    fn draw(&self,scene:&mut Scene,a:V3,b:V3,low:f64,high:f64){
        let length=scene.paper_mm((b-a).length());
        let samples=(length/2.5).ceil().clamp(8.0,400.0) as usize;
        let margin=self.step*0.25;
        for k in 0..self.layers.len(){
            let base=self.layers[k].base;
            if base<low-self.step||base>high+self.step{continue;}
            let mut run=Vec::new();
            for i in 0..=samples{
                let q=a+(b-a)*(i as f64/samples as f64);
                let (z,shown)=self.height(k,q.x,q.y);
                if shown&&z>low+margin&&z<high-margin{
                    run.push(v(q.x,q.y,z));
                    continue;
                }
                if run.len()>=2{scene.line(&run);}
                run.clear();
            }
            if run.len()>=2{scene.line(&run);}
        }
    }
}
/// Bancal: un bloque macizo de terreno en su franja, sin losas solapadas. Los
/// estratos solo van en las caras exteriores de la maqueta; la delantera que es un
/// muro de contención lleva albardilla y, en el dibujo final, hiladas muy abiertas.
pub fn terrace(scene:&mut Scene,style:Style,min:V3,max:V3,cut:Cut,strata:&Strata){
    if !dimensions(min,max){return;}
    part(scene,"terreno");
    scene.box3(min,max);
    let corners=[v(min.x,min.y,0.0),v(max.x,min.y,0.0),v(max.x,max.y,0.0),v(min.x,max.y,0.0)];
    for side in 0..4{
        let (a,b)=(corners[side],corners[(side+1)%4]);
        if cut.exterior[side]{
            strata.draw(scene,a,b,min.z,max.z);
        }else if side==0{
            if let Some(from)=cut.wall_from{retaining(scene,style,a,b,from.max(min.z),max.z,strata.step);}
        }
    }
}
/// Albardilla y, como mucho, hiladas muy abiertas en un muro de contención.
fn retaining(scene:&mut Scene,style:Style,a:V3,b:V3,low:f64,high:f64,step:f64){
    let coping=0.3f64.max(1.6/scene.camera().scale);
    let top=high-coping;
    if top-low<coping{return;}
    let at=|q:V3,z:f64|v(q.x,q.y,z);
    scene.line(&[at(a,top),at(b,top)]);
    if style.detail==Detail::Preview{return;}
    let course=(step*3.5).max(0.8);
    let count=((top-low)/course).floor() as usize;
    for j in 1..count{
        let z=low+(top-low)*j as f64/count as f64;
        scene.line(&[at(a,z),at(b,z)]);
    }
}
pub fn stairs(scene:&mut Scene,style:Style,min:V3,width:f64,run:f64,rise:f64){
    if !min.finite()||![width,run,rise].iter().all(|n|n.is_finite()&&*n>0.0){return;}
    // Una escalera de menos de 1,6 mm de huella total no se lee: su frente se
    // confundiría con el muro del bancal. Cada huella guarda también esa distancia.
    let apart=super::street::apart(scene);if run<apart*1.05{return;}
    let q=style.quantum(scene);let steps=(rise/q).round().min((run/(apart*1.05)).floor()).clamp(1.0,32.0) as usize;
    // Un solo prisma con el perfil escalonado: entre peldaños no queda ninguna junta.
    prism_piece(scene,style,&stair_profile(min,run,rise,steps),v(width,0.0,0.0),"terreno");
}
/// Perfil de una escalera en el plano x = min.x: suelo, frente alto y peldaños
/// de vuelta hasta el arranque.
fn stair_profile(min:V3,run:f64,rise:f64,steps:usize)->Vec<V3>{
    let y=|i:usize|min.y+run*i as f64/steps as f64;
    let z=|i:usize|min.z+rise*i as f64/steps as f64;
    let at=|y:f64,z:f64|v(min.x,y,z);
    let mut profile=vec![at(y(0),z(0)),at(y(steps),z(0)),at(y(steps),z(steps))];
    for i in (0..steps).rev(){
        profile.push(at(y(i),z(i+1)));
        if i>0{profile.push(at(y(i),z(i)));}
    }
    profile
}
pub fn vegetation(scene:&mut Scene,style:Style,origin:V3,height:f64,plant:Plant,rng:&mut Rng){
    if !origin.finite()||!height.is_finite()||height<=0.0||height>1000.0||!scene.visible(height,1.0){return;}
    let q=style.quantum(scene);let height=style.snap(scene,height).max(q*2.0);
    match plant{
        Plant::Cypress=>{
            // La punta se hunde claramente en el suelo; el encuentro queda contorneado.
            let radius=height*0.13;
            style.lathe(scene,origin,&[(radius*0.45,-q),(radius, height*0.2),(radius*0.85,height*0.65),(radius*0.4,height*0.9),(0.0,height)],"vegetación");
        }
        Plant::Olive=>{
            // El tronco acaba en punta dentro de la copa: no deja una tapa escondida.
            style.lathe(scene,origin,&[(height*0.055,0.0),(height*0.055,height*0.5),(0.0,height*0.56)],"vegetación");
            let radius=height*(0.3+rng.f()*0.06);
            let profile=[(0.0,height*0.35),(radius*0.8,height*0.5),(radius,height*0.7),
                (radius*0.7,height*0.9),(0.0,height)];
            style.lathe(scene,origin,&profile,"vegetación");
        }
        Plant::Shrub=>{
            let radius=height*(0.55+rng.f()*0.15);
            style.lathe(scene,origin,&[(radius,-q),(radius,height*0.35),(radius*0.65,height*0.8),(0.0,height)],"vegetación");
        }
        Plant::Palm=>palm(scene,style,origin,height,rng),
        Plant::Pine=>{
            // Tronco alto que acaba en punta dentro de la copa, como el del olivo.
            style.lathe(scene,origin,&[(height*0.035,0.0),(height*0.03,height*0.66),(0.0,height*0.72)],"vegetación");
            let radius=height*(0.42+rng.f()*0.06);
            let profile=[(0.0,height*0.62),(radius*0.75,height*0.66),(radius,height*0.78),(radius*0.8,height*0.9),(0.0,height*0.96)];
            style.lathe(scene,origin,&profile,"vegetación");
        }
    }
}
/// Palmera: tronco fino, un penacho que tapa el encuentro y ocho hojas en arco
/// que caen hacia fuera. Las hojas son trazos: no ocultan nada.
fn palm(scene:&mut Scene,style:Style,origin:V3,height:f64,rng:&mut Rng){
    let crown=height*0.92;
    style.lathe(scene,origin,&[(height*0.03,0.0),(height*0.022,crown),(0.0,crown+height*0.04)],"vegetación");
    let head=height*0.05;
    style.lathe(scene,origin+v(0.0,0.0,crown-head),&[(head*0.6,0.0),(head,head),(head*0.5,head*2.0),(0.0,head*2.4)],"vegetación");
    if !scene.visible(height*0.3,1.5){return;}
    part(scene,"vegetación");
    let top=origin+v(0.0,0.0,crown+head*0.8);
    let phase=rng.f()*std::f64::consts::TAU;
    for k in 0..8{
        let angle=phase+std::f64::consts::TAU*k as f64/8.0;
        let reach=height*(0.3+rng.f()*0.08);
        let (c,s)=(angle.cos(),angle.sin());
        let frond:Vec<V3>=(0..=5).map(|i|{
            let t=i as f64/5.0;
            let r=reach*t;
            let lift=height*(0.08*t*4.0*(1.0-t)-0.22*t*t);
            top+v(c*r,s*r,lift)
        }).collect();
        scene.line(&frond);
    }
}
